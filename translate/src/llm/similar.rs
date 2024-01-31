use once_cell::sync::Lazy;
use pyo3::{types::PyModule, Py, PyAny, PyResult, Python};

static SIMILAR_FN: Lazy<Py<PyAny>> = Lazy::new(|| Python::with_gil(|py| {
    PyModule::from_code(py, include_str!("similar.py"), "similar.py", "similar").unwrap().getattr("compute_similarity").unwrap().into()
}));

pub fn similar(py: Python, s1: &str, s2: Vec<&str>) -> PyResult<f64> {
    SIMILAR_FN.call1(py, (s1, s2))?.extract(py)
}
