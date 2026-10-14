import io
import json

from suna_worker.protocol import encode_frame, read_frame
from suna_worker.worker import serve
from suna_worker.training import _artifact_id


def submit(operation, payload):
    message = {"jsonrpc": "2.0", "protocol_version": "1.0", "id": "synthetic", "method": "submit",
               "params": {"task_id": "synthetic", "operation": operation, "payload": payload}}
    output = io.BytesIO()
    serve(io.BytesIO(encode_frame(message)), output)
    output.seek(0)
    frames = []
    while (frame := read_frame(output)) is not None:
        frames.append(frame)
    assert frames[-1]["id"] == "synthetic"
    return frames[-1]


def test_worker_training_reload_and_batch_risk_are_real(tmp_path):
    import suna_worker.training as training
    assert "compute-worker/suna_worker/training.py" in training.__file__.replace("\\", "/")
    payload = {"features": [[.1, 800], [.2, 810], [.3, 820], [.4, 830], [.5, 840], [.6, 850]],
               "targets": [100, 105, 110, 115, 120, 125], "feature_names": ["C", "temperature"],
               "data_version": "d" * 64, "split_policy": {"kind": "time", "validation_fraction": .33},
               "feature_metadata": [{"name": "C", "unit": "wt%"}, {"name": "temperature", "unit": "degC"}],
               "target_schema": {"name": "HV", "unit": "HV"}}
    training_response = submit("train_linear_regression", payload)
    assert "error" not in training_response
    artifact = training_response["result"]["artifact"]
    model_file = tmp_path / "中文 模型.json"
    model_file.write_text(json.dumps(artifact), encoding="utf-8")
    artifact = json.loads(model_file.read_text(encoding="utf-8"))
    assert artifact["model_id"] == _artifact_id(artifact)
    assert artifact["applicability_support"]["features"][1]["max"] == 830
    prediction = submit("predict_linear_regression", {"artifact": artifact, "features": [[.2, 810], [10, 1500]]})
    assert prediction["result"]["state"] == "completed"
    assert len(prediction["result"]["predictions"]) == 2
    assert [row["state"] for row in prediction["result"]["applicability"]["rows"]] == ["within_domain", "outside_domain"]
    assert prediction["result"]["applicability"]["probability"] is None


def test_worker_rejects_stale_hash_after_support_tamper():
    trained = submit("train_linear_regression", {"features": [[1], [2], [3]], "targets": [2, 4, 6],
                     "feature_names": ["time"], "data_version": "d" * 64,
                     "feature_metadata": [{"name": "time", "unit": "min"}]})
    artifact = trained["result"]["artifact"]
    artifact["applicability_support"]["features"][0]["max"] = 999
    response = submit("predict_linear_regression", {"artifact": artifact, "features": [[2]]})
    assert response["error"]["code"] == "invalid_payload"
    assert response["error"]["message"] == "model_identity_mismatch"
