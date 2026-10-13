import pytest
from suna_worker.evaluation import train_evaluated


def payload(kind="group"):
    return {"features":[[i/10,i%7] for i in range(240)],"targets":[2*i/10+(i%7)*.4+((i*13)%11)*.1 for i in range(240)],"feature_names":["C","temp"],"groups":[str(i//12) for i in range(240)],"timestamps":list(range(240)),"split_policy":{"kind":kind,"seed":42,"validation_fraction":.2},"evaluation":{"folds":3,"test_fraction":.2,"calibration_fraction":.2,"learning_curve":True}}


def test_group_cv_holdouts_and_calibration_do_not_leak():
    p=payload();a=train_evaluated(p,linear=True)
    partitions=[set(a["split"][k]) for k in ("train_indices","validation_indices","test_indices","calibration_indices")]
    assert set.union(*partitions)==set(range(240))
    for i,x in enumerate(partitions):
        for y in partitions[i+1:]:
            assert not x&y
            assert not {p["groups"][j] for j in x}&{p["groups"][j] for j in y}
    for fold in a["evaluation"]["folds"]:
        assert not {p["groups"][j] for j in fold["train_indices"]}&{p["groups"][j] for j in fold["validation_indices"]}
    assert len(a["evaluation"]["learning_curve"])==3
    assert a["conformal"]["sample_count"]>=19
    assert 0<=a["evaluation"]["test_interval_coverage"]<=1


def test_time_cv_and_test_are_future_only():
    a=train_evaluated(payload("time"),linear=True)
    for fold in a["evaluation"]["folds"]:
        assert max(fold["train_indices"])<min(fold["validation_indices"])
    assert max(a["split"]["train_indices"])<min(a["split"]["calibration_indices"])
    assert max(a["split"]["calibration_indices"])<min(a["split"]["test_indices"])
    assert a["evaluation"]["test_extrapolation_count"]>0


def test_train_only_preprocessing_and_no_false_small_sample_interval():
    p=payload("random");p["evaluation"]={"folds":0,"test_fraction":.2,"calibration_fraction":.02}
    a=train_evaluated(p,linear=True);training=a["split"]["train_indices"]
    assert a["preprocessing"]["means"][0]==pytest.approx(sum(p["features"][i][0] for i in training)/len(training))
    assert "conformal" not in a
    assert a["evaluation"]["test_interval_coverage"] is None


def test_hyperparameter_search_uses_cv_and_reports_parameters():
    p=payload();p["algorithm"]="svr";p["evaluation"].update(search_trials=2,learning_curve=False)
    a=train_evaluated(p)
    assert len(a["evaluation"]["folds"])==3
    assert isinstance(a["evaluation"]["selected_parameters"],dict)
    p["evaluation"]["folds"]=0
    with pytest.raises(ValueError,match="cross-validation"):train_evaluated(p)


def test_scientific_training_runs_through_worker_protocol():
    import io
    from suna_worker.protocol import encode_frame, read_frame
    from suna_worker.worker import serve
    def frame(id, method, params):
        return encode_frame({"jsonrpc":"2.0","protocol_version":"1.0","id":id,"method":method,"params":params})
    output=io.BytesIO()
    serve(io.BytesIO(frame("science","submit",{"task_id":"science-task","operation":"train_linear_regression","payload":payload()})+frame("shutdown","shutdown",{})),output)
    output.seek(0)
    messages=[]
    while (message:=read_frame(output)) is not None:
        messages.append(message)
    result=next(m["result"] for m in messages if m.get("id")=="science")
    assert result["state"]=="completed"
    assert result["artifact"]["metrics"]["test"]["sample_count"]>0
    assert result["artifact"]["conformal"]["sample_count"]>=19
