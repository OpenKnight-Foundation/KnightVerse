use crate::helper::password;
use chrono::Utc;
use db_entity::user;
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter,
};

/// User service for authentication and user management
pub struct UserService;

impl UserService {
    /// Register a new user
    pub async fn register(
        db: &DatabaseConnection,
        username: &str,
        email: &str,
        password: &str,
    ) -> Result<user::Model, DbErr> {
        // Check if username already exists
        let existing_user = user::Entity::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?;

        if existing_user.is_some() {
            return Err(DbErr::Custom("Username already exists".to_string()));
        }

        // Check if email already exists
        let existing_email = user::Entity::find()
            .filter(user::Column::Email.eq(email))
            .one(db)
            .await?;

        if existing_email.is_some() {
            return Err(DbErr::Custom("Email already exists".to_string()));
        }

        let password_hash = password::hash_password(password)
            .map_err(|_| DbErr::Custom("Failed to hash password".to_string()))?;

        let now = Utc::now();

        // Create new user
        let new_user = user::ActiveModel {
            username: ActiveValue::Set(username.to_string()),
            email: ActiveValue::Set(email.to_string()),
            password_hash: ActiveValue::Set(password_hash),
            created_at: ActiveValue::Set(now),
            updated_at: ActiveValue::Set(now),
            ..Default::default()
        };

        let user = new_user.insert(db).await?;
        Ok(user)
    }

    /// Authenticate user with username and password
    pub async fn authenticate(
        db: &DatabaseConnection,
        username: &str,
        password: &str,
    ) -> Result<user::Model, DbErr> {
        let user = user::Entity::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await?;

        match user {
            Some(user_model) => {
                match password::verify_password(password, &user_model.password_hash) {
                    Ok(()) => Ok(user_model),
                    Err(_) => Err(DbErr::Custom("Invalid password".to_string())),
                }
            }
            None => Err(DbErr::Custom("User not found".to_string())),
        }
    }

    /// Get user by ID
    pub async fn get_by_id(
        db: &DatabaseConnection,
        user_id: i32,
    ) -> Result<Option<user::Model>, DbErr> {
        user::Entity::find_by_id(user_id).one(db).await
    }

    /// Get user by username
    pub async fn get_by_username(
        db: &DatabaseConnection,
        username: &str,
    ) -> Result<Option<user::Model>, DbErr> {
        user::Entity::find()
            .filter(user::Column::Username.eq(username))
            .one(db)
            .await
    }

    /// Get user by email
    pub async fn get_by_email(
        db: &DatabaseConnection,
        email: &str,
    ) -> Result<Option<user::Model>, DbErr> {
        user::Entity::find()
            .filter(user::Column::Email.eq(email))
            .one(db)
            .await
    }
}
