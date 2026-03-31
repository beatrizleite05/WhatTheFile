pub struct Chunk {
    pub start_token: usize,
    pub end_token: usize,
    pub text: String,
}

pub fn chunk_text(_text: &str, _chunk_size: usize, _overlap: usize) -> Vec<Chunk> {
    todo!("Phase C: implement 256/32 sliding window chunker")
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder() {}
}
