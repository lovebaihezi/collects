use super::{CollectTag, Tag};
use sqlx::PgPool;

//
// Tag CRUD
//

pub async fn create_tag(pool: &PgPool, author_id: &str, name: &str) -> Result<Tag, sqlx::Error> {
    let tag = sqlx::query_as!(
        Tag,
        r#"
        INSERT INTO tags (author_id, name)
        VALUES ($1, $2)
        RETURNING *
        "#,
        author_id,
        name
    )
    .fetch_one(pool)
    .await?;

    Ok(tag)
}

pub async fn get_tag_by_id(pool: &PgPool, id: i32) -> Result<Option<Tag>, sqlx::Error> {
    let tag = sqlx::query_as!(
        Tag,
        r#"
        SELECT * FROM tags
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(tag)
}

pub async fn get_tags_by_author(pool: &PgPool, author_id: &str) -> Result<Vec<Tag>, sqlx::Error> {
    let tags = sqlx::query_as!(
        Tag,
        r#"
        SELECT * FROM tags
        WHERE author_id = $1 AND deleted_at IS NULL
        ORDER BY created_at DESC
        "#,
        author_id
    )
    .fetch_all(pool)
    .await?;

    Ok(tags)
}

pub async fn delete_tag(pool: &PgPool, id: i32) -> Result<Tag, sqlx::Error> {
    let tag = sqlx::query_as!(
        Tag,
        r#"
        UPDATE tags
        SET deleted_at = now()
        WHERE id = $1
        RETURNING *
        "#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(tag)
}

//
// CollectTag (join table) CRUD
//

pub async fn add_tag_to_collect(
    pool: &PgPool,
    collect_id: i32,
    tag_id: i32,
) -> Result<CollectTag, sqlx::Error> {
    let collect_tag = sqlx::query_as!(
        CollectTag,
        r#"
        INSERT INTO collect_tags (collect_id, tag_id)
        VALUES ($1, $2)
        RETURNING *
        "#,
        collect_id,
        tag_id
    )
    .fetch_one(pool)
    .await?;

    Ok(collect_tag)
}

pub async fn remove_tag_from_collect(
    pool: &PgPool,
    collect_id: i32,
    tag_id: i32,
) -> Result<CollectTag, sqlx::Error> {
    let collect_tag = sqlx::query_as!(
        CollectTag,
        r#"
        UPDATE collect_tags
        SET deleted_at = now()
        WHERE collect_id = $1 AND tag_id = $2
        RETURNING *
        "#,
        collect_id,
        tag_id
    )
    .fetch_one(pool)
    .await?;

    Ok(collect_tag)
}

pub async fn get_tags_for_collect(pool: &PgPool, collect_id: i32) -> Result<Vec<Tag>, sqlx::Error> {
    let tags = sqlx::query_as!(
        Tag,
        r#"
        SELECT t.id, t.author_id, t.name, t.created_at, t.deleted_at
        FROM tags t
        INNER JOIN collect_tags ct ON t.id = ct.tag_id
        WHERE ct.collect_id = $1
          AND t.deleted_at IS NULL
          AND ct.deleted_at IS NULL
        "#,
        collect_id
    )
    .fetch_all(pool)
    .await?;

    Ok(tags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collects::PrivacyKind;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    // Helper struct to hold IDs for testing
    struct TestSetup {
        pool: PgPool,
        author_id: String,
        collect_id: i32,
    }

    async fn setup() -> TestSetup {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Failed to create pool.");

        // Clean tables
        sqlx::query("DELETE FROM collect_tags")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM tags")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM collects")
            .execute(&pool)
            .await
            .unwrap();

        let author_id = "test_author_for_tags";
        let collect = sqlx::query!(
            r#"
            INSERT INTO collects (author_id, content, privacy_level)
            VALUES ($1, $2, $3)
            RETURNING id
            "#,
            author_id,
            "dummy content for tags",
            PrivacyKind::Public as _
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        TestSetup {
            pool,
            author_id: author_id.to_string(),
            collect_id: collect.id,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_tag() {
        let ts = setup().await;
        let tag_name = "test_tag";

        let created_tag = create_tag(&ts.pool, &ts.author_id, tag_name).await.unwrap();
        assert_eq!(created_tag.name, tag_name);
        assert_eq!(created_tag.author_id, ts.author_id);

        let fetched_tag = get_tag_by_id(&ts.pool, created_tag.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(created_tag, fetched_tag);

        let author_tags = get_tags_by_author(&ts.pool, &ts.author_id).await.unwrap();
        assert_eq!(author_tags.len(), 1);
        assert_eq!(author_tags[0], created_tag);
    }

    #[tokio::test]
    async fn test_delete_tag() {
        let ts = setup().await;
        let tag = create_tag(&ts.pool, &ts.author_id, "to_delete")
            .await
            .unwrap();

        let deleted_tag = delete_tag(&ts.pool, tag.id).await.unwrap();
        assert!(deleted_tag.deleted_at.is_some());

        let fetched_tag = get_tag_by_id(&ts.pool, tag.id).await.unwrap();
        assert!(fetched_tag.is_none());
    }

    #[tokio::test]
    async fn test_tag_collect_associations() {
        let ts = setup().await;
        let tag1 = create_tag(&ts.pool, &ts.author_id, "tag1").await.unwrap();
        let tag2 = create_tag(&ts.pool, &ts.author_id, "tag2").await.unwrap();

        // 1. Add tags to collect
        add_tag_to_collect(&ts.pool, ts.collect_id, tag1.id)
            .await
            .unwrap();
        add_tag_to_collect(&ts.pool, ts.collect_id, tag2.id)
            .await
            .unwrap();

        // 2. Get tags for collect
        let collect_tags = get_tags_for_collect(&ts.pool, ts.collect_id).await.unwrap();
        assert_eq!(collect_tags.len(), 2);
        assert!(collect_tags.contains(&tag1));
        assert!(collect_tags.contains(&tag2));

        // 3. Remove a tag from collect
        remove_tag_from_collect(&ts.pool, ts.collect_id, tag1.id)
            .await
            .unwrap();

        // 4. Verify removal
        let collect_tags_after_removal =
            get_tags_for_collect(&ts.pool, ts.collect_id).await.unwrap();
        assert_eq!(collect_tags_after_removal.len(), 1);
        assert_eq!(collect_tags_after_removal[0], tag2);
    }
}
