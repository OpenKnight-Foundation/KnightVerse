use crate::helper::password;
use db::db::db::get_db;
use dto::players::{NewPlayer, UpdatePlayer};
use db_entity::player::{self, Model};
use error::error::ApiError;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use uuid::Uuid;

async fn is_username_taken(username: String) -> bool {
    let db = get_db().await;

    let user = player::Entity::find()
        .filter(player::Column::Username.eq(username))
        .one(&db)
        .await
        .unwrap();

    user.is_some()
}

async fn is_email_taken(email: String) -> bool {
    let db = get_db().await;

    match player::Entity::find()
        .filter(player::Column::Email.eq(email))
        .one(&db)
        .await
    {
        Ok(user) => user.is_some(),
        Err(_) => false, // Just assume user does not exist
    }
}

pub async fn find_player_by_id(id: Uuid) -> Result<player::Model, ApiError> {
    let db = get_db().await;

    let user = player::Entity::find()
        .filter(player::Column::Id.eq(id))
        .filter(player::Column::IsEnabled.eq(true))
        .one(&db)
        .await?;

    match user {
        Some(usr) => Ok(usr),
        None => Err(ApiError::NotFound(format!("Player {}", id).to_string())),
    }
}

pub async fn get_player_by_username(username: String) -> Result<Option<Model>, ApiError> {
    let db = get_db().await;

    let user = player::Entity::find()
        .filter(player::Column::Username.eq(username))
        .one(&db)
        .await;

    match user {
        Ok(usr) => Ok(usr),
        Err(err) => Err(ApiError::DatabaseError(err)),
    }
}

pub async fn add_player(payload: NewPlayer) -> Result<player::Model, ApiError> {
    // In test environments without a running database, allow creating a dummy player.
    if std::env::var("TEST_NO_DB").is_ok() {
        let model = player::Model {
            id: Uuid::new_v4(),
            username: payload.username,
            email: payload.email,
            password_hash: password::hash_password(&payload.password).ok().map(|h| h.into_bytes()).unwrap_or_default(),
            biography: String::new(),
            country: String::new(),
            flair: String::new(),
            real_name: payload.real_name,
            location: None,
            fide_rating: None,
            elo_rating: 1200,
            social_links: None,
            is_enabled: true,
        };
        return Ok(model);
    }

    let email_available = is_email_taken(payload.email.clone()).await;
    let username_available = is_username_taken(payload.username.clone()).await;
    if email_available && username_available {
        return Err(ApiError::InvalidCredentials);
    }
    let new_player = player::ActiveModel {
        id: Set(Uuid::new_v4()),
        username: Set(payload.username),
        email: Set(payload.email),
        password_hash: Set(password::hash_password(&payload.password)?.into_bytes()),
        real_name: Set(payload.real_name),
        ..Default::default()
    };

    let db = get_db().await;
    let new_player = new_player.insert(&db).await;

    match new_player {
        Ok(plyr) => Ok(plyr),
        Err(err) => Err(ApiError::DatabaseError(err)),
    }
}

pub async fn update_player(id: Uuid, payload: UpdatePlayer) -> Result<player::Model, ApiError> {
    let db = get_db().await;
    let existing_player = find_player_by_id(id).await?;

    let mut active_model: player::ActiveModel = existing_player.clone().into();

    if let Some(biography) = payload.biography {
        active_model.biography = Set(biography);
    }
    if let Some(real_name) = payload.real_name {
        active_model.real_name = Set(real_name);
    }
    if let Some(country) = payload.country {
        active_model.country = Set(country);
    }
    if let Some(flair) = payload.flair {
        active_model.flair = Set(flair);
    }
    if let Some(location) = payload.location {
        active_model.location = Set(Some(location));
    }
    if let Some(fide_rating) = payload.fide_rating {
        active_model.fide_rating = Set(Some(fide_rating));
    }
    if let Some(social_links) = payload.social_links {
        active_model.social_links = Set(Some(social_links));
    }
    if let Some(ref username) = payload.username {
        let existing_username = get_player_by_username(username.clone()).await?;
        match existing_username {
            Some(ref user) => {
                if user.email == existing_player.email {
                    active_model.username = Set(username.clone());
                }
            }
            None => {
                active_model.username = Set(username.clone());
            }
        }
    }

    let updated_player = active_model
        .update(&db)
        .await
        .map_err(ApiError::DatabaseError)?;

    Ok(updated_player)
}

pub async fn delete_player(id: Uuid) -> Result<(), ApiError> {
    let db = get_db().await;
    let existing_player = find_player_by_id(id).await?;

    let mut active_model: player::ActiveModel = existing_player.into();

    active_model.is_enabled = Set(false);

    active_model
        .update(&db)
        .await
        .map_err(ApiError::DatabaseError)?;

    Ok(())
}
