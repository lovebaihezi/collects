use super::CollectFile;
use sqlx::PgPool;

pub async fn create_collect_file(
    pool: &PgPool,
    author_id: &str,
    collect_id: i32,
    file_url: &str,
) -> Result<CollectFile, sqlx::Error> {
    let file = sqlx::query_as!(
        CollectFile,
        r#"
        INSERT INTO collect_files (author_id, collect_id, file_url)
        VALUES ($1, $2, $3)
        RETURNING *
        "#,
        author_id,
        collect_id,
        file_url
    )
    .fetch_one(pool)
    .await?;

    Ok(file)
}

pub async fn get_collect_files_by_collect_id(
    pool: &PgPool,
    collect_id: i32,
) -> Result<Vec<CollectFile>, sqlx::Error> {
    let files = sqlx::query_as!(
        CollectFile,
        r#"
        SELECT * FROM collect_files
        WHERE collect_id = $1 AND deleted_at IS NULL
        "#,
        collect_id
    )
    .fetch_all(pool)
    .await?;

    Ok(files)
}

pub async fn get_collect_file_by_id(
    pool: &PgPool,
    id: i32,
) -> Result<Option<CollectFile>, sqlx::Error> {
    let file = sqlx::query_as!(
        CollectFile,
        r#"
        SELECT * FROM collect_files
        WHERE id = $1 AND deleted_at IS NULL
        "#,
        id
    )
    .fetch_optional(pool)
    .await?;

    Ok(file)
}

pub async fn delete_collect_file(pool: &PgPool, id: i32) -> Result<CollectFile, sqlx::Error> {
    let file = sqlx::query_as!(
        CollectFile,
        r#"
        UPDATE collect_files
        SET deleted_at = now()
        WHERE id = $1
        RETURNING *
        "#,
        id
    )
    .fetch_one(pool)
    .await?;

    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;
    use std::env;

    // Helper struct to insert a collect for FK constraints
    struct DummyCollect {
        id: i32,
        author_id: String,
    }

    async fn setup() -> (PgPool, DummyCollect) {
        let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .connect(&database_url)
            .await
            .expect("Failed to create pool.");

        // Clean tables to ensure a fresh state
        sqlx::query("DELETE FROM collect_files")
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM collects")
            .execute(&pool)
            .await
            .unwrap();

        // Insert a dummy collect to satisfy the foreign key constraint
        let author_id = "test_author_for_files";
        let collect = sqlx::query!(
            r#"
            INSERT INTO collects (author_id, content, privacy_level)
            VALUES ($1, $2, $3)
            RETURNING id, author_id
            "#,
            author_id,
            "dummy content",
            PrivacyKind::Public as _
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let dummy_collect = DummyCollect {
            id: collect.id,
            author_id: collect.author_id,
        };

        (pool, dummy_collect)
    }

    #[tokio::test]
    async fn test_create_and_get_collect_file() {
        let (pool, dummy_collect) = setup().await;
        let file_url = "https://example.com/file1.jpg";

        // 1. Test creation
        let created_file =
            create_collect_file(&pool, &dummy_collect.author_id, dummy_collect.id, file_url)
                .await
                .unwrap();

        assert_eq!(created_file.author_id, dummy_collect.author_id);
        assert_eq!(created_file.collect_id, dummy_collect.id);
        assert_eq!(created_file.file_url, file_url);
        assert!(created_file.deleted_at.is_none());

        // 2. Test retrieval by ID
        let fetched_file = get_collect_file_by_id(&pool, created_file.id)
            .await
            .unwrap()
            .expect("File should be found");

        assert_eq!(created_file, fetched_file);

        // 3. Test retrieval by collect_id
        let files_for_collect = get_collect_files_by_collect_id(&pool, dummy_collect.id)
            .await
            .unwrap();

        assert_eq!(files_for_collect.len(), 1);
        assert_eq!(files_for_collect[0], created_file);
    }

    #[tokio::test]
    async fn test_delete_collect_file() {
        let (pool, dummy_collect) = setup().await;
        let file_url = "https://example.com/file_to_delete.jpg";

        let created_file =
            create_collect_file(&pool, &dummy_collect.author_id, dummy_collect.id, file_url)
                .await
                .unwrap();

        // Delete the file
        let deleted_file = delete_collect_file(&pool, created_file.id).await.unwrap();
        assert!(deleted_file.deleted_at.is_some());

        // Verify it's no longer retrievable by ID
        let fetched_file = get_collect_file_by_id(&pool, created_file.id)
            .await
            .unwrap();
        assert!(fetched_file.is_none());

        // Verify it's no longer retrieved with its collect
        let files_for_collect = get_collect_files_by_collect_id(&pool, dummy_collect.id)
            .await
            .unwrap();
        assert!(files_for_collect.is_empty());
    }
}
