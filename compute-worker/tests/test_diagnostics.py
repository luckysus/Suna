import io
from suna_worker import diagnostics
from suna_worker.worker import serve
from suna_worker.protocol import encode_frame, read_frame


def test_missing_native_dependency_is_degraded_and_does_not_leak_error(monkeypatch):
    def load(module):
        if module == "PIL.Image":
            raise OSError("secret endpoint C:/private/file.dll")
        return object()
    monkeypatch.setattr(diagnostics, "import_module", load)
    report = diagnostics.dependency_report()
    assert report["state"] == "degraded"
    assert not report["features"]["microstructure"]
    assert report["features"]["prediction"]
    assert "secret" not in str(report)
    assert not report["networkUsed"]


def test_probe_uses_real_framed_protocol_and_imports_installed_dependencies():
    def req(method):
        return encode_frame({"jsonrpc": "2.0", "protocol_version": "1.0", "id": method, "method": method, "params": {}})
    out = io.BytesIO()
    serve(io.BytesIO(req("diagnose") + req("shutdown")), out)
    out.seek(0)
    result = read_frame(out)["result"]
    assert result["protocolVersion"] == "1.0"
    assert result["features"]["prediction"]
    assert result["features"]["microstructure"]
    assert {r["name"] for r in result["dependencies"]} == set(diagnostics.DEPENDENCIES)
