use argon2::{password_hash::SaltString, Argon2, PasswordHash, PasswordHasher, PasswordVerifier};

use rand::rngs::OsRng;

pub fn hash_password(password: &str) -> Result<String, argon2::password_hash::Error> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default();
    let hash = argon2.hash_password(password.as_bytes(), &salt)?; // ? propagates errors
    Ok(hash.to_string())
}

pub fn verify_password<'a>(
    password: &'a str,
    hashed_password: &'a str,
) -> Result<(), argon2::password_hash::Error> {
    let password_hash = PasswordHash::new(hashed_password)?;
    // Trait objects for algorithms to support
    let algs: &[&dyn PasswordVerifier] = &[&Argon2::default()];

    match password_hash.verify_password(algs, password) {
        Ok(_) => Ok(()),
        Err(_err) => Err(_err),
    }
}
