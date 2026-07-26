pub struct Auth {
    token: String,
}

impl Auth {
    pub fn new(token: String) -> Self {
        Self { token }
    }

    pub fn validate(&self, request_token: &str) -> bool {
        self.token == request_token
    }

    pub fn token(&self) -> &str {
        &self.token
    }
}

pub fn generate_random_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32).map(|_| format!("{:x}", rng.gen::<u8>() % 16)).collect()
}
