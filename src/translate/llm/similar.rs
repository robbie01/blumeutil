use once_cell::sync::Lazy;
use pyo3::{types::PyModule, Py, PyAny, Python};

static SIMILAR_FN: Lazy<Py<PyAny>> = Lazy::new(|| Python::with_gil(|py| {
    PyModule::from_code(py, include_str!("similar.py"), "similar.py", "similar").unwrap().getattr("compute_similarity").unwrap().into()
}));

pub fn similar(s1: &str, s2: Vec<&str>) -> anyhow::Result<Vec<f64>> {
    Ok(Python::with_gil(|py| SIMILAR_FN.call1(py, (s1, s2))?.extract(py))?)
}
