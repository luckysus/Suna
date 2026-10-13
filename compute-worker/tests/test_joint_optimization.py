import pytest
from suna_worker.training import train_linear_regression, train_sklearn_model
from suna_worker.optimization import optimize_constrained, OptimizationError


def linear(names, coefficients):
    rows = [[i/10, (i % 7)/3][:len(names)] for i in range(30)]
    return train_linear_regression({"feature_names":names,"features":rows,"targets":[sum(v*c for v,c in zip(r,coefficients)) for r in rows]})


def request():
    return {"feature_names":["C","temp"],"bounds":[{"min":0,"max":1}]*2,"fixed_values":{},"constraints":[],"trials":25,"seed":42,"algorithm":"grid",
        "objective_specs":[{"name":"HV","source":"prediction_model","direction":"maximize","unit":"HV","artifact":linear(["C","temp"],[1,2])},
        {"name":"J","source":"prediction_model","direction":"maximize","unit":"J","artifact":linear(["temp","C"],[-1,1])},
        {"name":"cost","source":"formula","direction":"minimize","unit":"CNY","coefficients":{"temp":1},"intercept":5}]}


def test_independent_model_feature_mapping_and_mixed_direction_pareto():
    result=optimize_constrained(request())
    assert result["directions"]==["maximize","maximize","minimize"]
    assert len(result["pareto_front"])>1
    for r in result["pareto_front"]:
        c,t=r["values"]["C"],r["values"]["temp"]
        assert r["objectives"]==pytest.approx([c+2*t,c-t,5+t],abs=1e-6)
        assert c==pytest.approx(1)
    vectors=[[-r["objectives"][0],-r["objectives"][1],r["objectives"][2]] for r in result["pareto_front"]]
    assert not any(all(a<=b for a,b in zip(x,y)) and any(a<b for a,b in zip(x,y)) for x in vectors for y in vectors)


def test_fixed_values_survive_equality_projection_and_all_fixed_search():
    p=request();p["fixed_values"]={"C":.4};p["constraints"]=[{"kind":"equality","coefficients":{"C":1,"temp":1},"value":1,"tolerance":1e-8}]
    for r in optimize_constrained(p)["pareto_front"]:
        assert r["values"]==pytest.approx({"C":.4,"temp":.6})
    p["fixed_values"]["temp"]=.6
    assert optimize_constrained(p)["trials_completed"]==1


def test_nsga2_accepts_independent_families_and_is_deterministic():
    p=request();p["algorithm"]="nsga2";p["trials"]=20
    for o,algorithm in zip(p["objective_specs"][:2],["random_forest","svr"]):
        o["artifact"]=train_sklearn_model({"features":[[i/10] for i in range(30)],"targets":[i/10 for i in range(30)],"feature_names":["C"],"algorithm":algorithm,"n_estimators":10})
    assert optimize_constrained(p)["pareto_front"]==optimize_constrained(p)["pareto_front"]


def test_unknown_formula_and_missing_model_features_rejected():
    p=request();p["objective_specs"][2]["coefficients"]={"unknown":1}
    with pytest.raises(OptimizationError):optimize_constrained(p)
    p=request();p["feature_names"]=["C"]
    with pytest.raises(OptimizationError):optimize_constrained(p)


def test_joint_optimization_runs_through_framed_worker_protocol():
    import io
    from suna_worker.protocol import encode_frame, read_frame
    from suna_worker.worker import serve
    def frame(id, method, params):
        return encode_frame({"jsonrpc":"2.0","protocol_version":"1.0","id":id,"method":method,"params":params})
    output=io.BytesIO()
    serve(io.BytesIO(frame("joint","submit",{"task_id":"joint-task","operation":"optimize_constrained","payload":request()})+frame("shutdown","shutdown",{})),output)
    output.seek(0)
    messages=[]
    while (message:=read_frame(output)) is not None:
        messages.append(message)
    result=next(m["result"] for m in messages if m.get("id")=="joint")
    assert result["state"]=="completed"
    assert result["directions"]==["maximize","maximize","minimize"]
    assert len(result["pareto_front"])>1
