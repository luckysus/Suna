"""Explicit offline dependency probe. Never report paths, environments or secrets."""
from importlib import import_module, metadata

DEPENDENCIES = {
    "numpy": "numpy", "scikit-learn": "sklearn", "scipy": "scipy",
    "pillow": "PIL.Image", "optuna": "optuna", "xgboost": "xgboost",
    "lightgbm": "lightgbm", "onnx": "onnx", "onnxruntime": "onnxruntime",
}


def dependency_report() -> dict:
    dependencies = []
    for package, module in DEPENDENCIES.items():
        try:
            import_module(module)  # Loading catches missing native libraries too.
            try:
                version = metadata.version(package)
            except metadata.PackageNotFoundError:
                version = None  # Frozen Worker can omit distribution metadata.
            dependencies.append({"name": package, "status": "ready", "version": version, "errorCode": None})
        except ModuleNotFoundError:
            dependencies.append({"name": package, "status": "unavailable", "version": None, "errorCode": "dependency_missing"})
        except Exception:
            dependencies.append({"name": package, "status": "unavailable", "version": None, "errorCode": "dependency_import_failed"})
    ready = {row["name"] for row in dependencies if row["status"] == "ready"}
    return {"state": "ready" if len(ready) == len(DEPENDENCIES) else "degraded",
            "dependencies": dependencies,
            "features": {"prediction": {"numpy", "scikit-learn", "scipy"} <= ready,
                         "jointOptimization": {"numpy", "scikit-learn", "optuna"} <= ready,
                         "microstructure": {"numpy", "pillow", "scipy"} <= ready,
                         "onnxInference": "onnxruntime" in ready},
            "networkUsed": False}
