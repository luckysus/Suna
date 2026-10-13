"""All values below are synthetic algorithm fixtures, not experimental facts."""
import math
import numpy as np
import pytest
from suna_worker.design import design_experiments, DesignError


def payload(strategy):
    xs=np.linspace(0.0,1.0,8)
    return {'method':'active_learning' if strategy=='exploration' else 'bayesian','variables':[{'name':'C','low':0,'high':1}], 'existing':{'feature_names':['C'],'features':[[float(x)] for x in xs],'targets':[float(400+50*x) for x in xs]}, 'count':3,'seed':42,'direction':'maximize','recommendation_policy':{'strategy':strategy,'min_distance':0.02,'constraints':[{'kind':'inequality','coefficients':{'C':1},'value':0.25},{'kind':'inequality','coefficients':{'C':-1},'value':-0.75}],'exclude':[{'C':0.5}]}}


@pytest.mark.parametrize('strategy',['exploitation','exploration','balanced'])
def test_budget_constraints_dedup_and_honest_gp_uncertainty(strategy):
    result=design_experiments(payload(strategy))
    assert len(result['points'])==3
    values=[p['values']['C'] for p in result['points']]
    assert all(0.25<=v<=0.75 and abs(v-0.5)>0.02 for v in values)
    assert all(abs(a-b)>0.02 for i,a in enumerate(values) for b in values[i+1:])
    assert all(math.isfinite(p['predicted_std']) and p['predicted_std']>=0 and 'GP' in p['uncertainty_source'] for p in result['points'])
    assert 'not the uncertainty' in result['recommendation_policy']['uncertainty_source']
    assert result==design_experiments(payload(strategy))


def test_infeasible_space_and_invalid_strategy_fail_explicitly():
    request=payload('balanced')
    request['recommendation_policy']['constraints']=[{'kind':'inequality','coefficients':{'C':1},'value':2}]
    with pytest.raises(DesignError,match='leave no candidate'):
        design_experiments(request)
    request=payload('balanced')
    request['recommendation_policy']['strategy']='fake-uncertainty'
    with pytest.raises(DesignError,match='strategy'):
        design_experiments(request)
