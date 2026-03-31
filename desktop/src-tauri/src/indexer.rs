use crate::errors::AppError;

pub async fn run_index_job(_root_path: &str) -> Result<(), AppError> {
    todo!("Phase B: implement two-phase incremental scan")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
