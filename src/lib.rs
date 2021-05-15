use crate::models::search::{DatabaseQuery, SearchRequest};
use crate::models::{Database, DatabaseId, ListResponse, Object, Page};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{header, Client, ClientBuilder, RequestBuilder};
use serde::de::DeserializeOwned;

mod models;

const NOTION_API_VERSION: &'static str = "2021-05-13";

// todo: replace with proper snafu error
pub type NotionApiClientError = Box<dyn std::error::Error>;

trait Identifiable {
    // There should only be one way to identify an object
    type Type;
    fn id(&self) -> &Self::Type;
}

struct NotionApi {
    client: Client,
}

impl NotionApi {
    pub fn new(api_token: String) -> Result<Self, NotionApiClientError> {
        let mut headers = HeaderMap::new();
        headers.insert(
            "Notion-Version",
            HeaderValue::from_static(NOTION_API_VERSION),
        );

        let mut auth_value = HeaderValue::from_str(&format!("Bearer {}", api_token))?;
        auth_value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, auth_value);

        let client = ClientBuilder::new().default_headers(headers).build()?;

        Ok(Self { client })
    }

    async fn make_json_request<T>(request: RequestBuilder) -> Result<T, NotionApiClientError>
    where
        T: DeserializeOwned,
    {
        let json = request.send().await?.text().await?;
        println!("JSON: {}", json);
        dbg!(serde_json::from_str::<serde_json::Value>(&json)?);
        let result = serde_json::from_str(&json)?;
        Ok(result)
    }

    /// This method is apparently deprecated/"not recommended"
    pub async fn list_databases(
        &self,
    ) -> Result<ListResponse<Database>, Box<dyn std::error::Error>> {
        let builder = self.client.get("https://api.notion.com/v1/databases");

        Ok(NotionApi::make_json_request(builder).await?)
    }

    pub async fn search<T: Into<SearchRequest>>(
        &self,
        query: T,
    ) -> Result<ListResponse<Database>, NotionApiClientError> {
        Ok(NotionApi::make_json_request(
            self.client
                .post("https://api.notion.com/v1/search")
                .json(&query.into()),
        )
        .await?)
    }

    pub async fn get_database<T: Identifiable<Type = DatabaseId>>(
        &self,
        database_id: T,
    ) -> Result<Database, NotionApiClientError> {
        Ok(NotionApi::make_json_request(self.client.get(format!(
            "https://api.notion.com/v1/databases/{}",
            database_id.id().id()
        )))
        .await?)
    }

    pub async fn query_database<D, T>(
        &self,
        database: D,
        query: T,
    ) -> Result<ListResponse<Page>, NotionApiClientError>
    where
        T: Into<DatabaseQuery>,
        D: Identifiable<Type = DatabaseId>,
    {
        Ok(NotionApi::make_json_request(
            self.client
                .post(&format!(
                    "https://api.notion.com/v1/databases/{database_id}/query",
                    database_id = database.id()
                ))
                .json(&query.into()),
        )
        .await?)
    }
}

#[cfg(test)]
mod tests {
    use crate::models::search::PropertyCondition::Text;
    use crate::models::search::{
        DatabaseQuery, FilterCondition, FilterProperty, FilterValue, NotionSearch, TextCondition,
    };
    use crate::{Identifiable, NotionApi};

    const TEST_TOKEN: &'static str = include_str!(".api_token");

    fn test_client() -> NotionApi {
        NotionApi::new(TEST_TOKEN.trim().to_string()).unwrap()
    }

    #[tokio::test]
    async fn list_databases() -> Result<(), Box<dyn std::error::Error>> {
        let api = test_client();

        dbg!(api.list_databases().await?);

        Ok(())
    }

    #[tokio::test]
    async fn search() -> Result<(), Box<dyn std::error::Error>> {
        let api = test_client();

        dbg!(
            api.search(NotionSearch::Filter {
                value: FilterValue::Database,
                property: FilterProperty::Object
            })
            .await?
        );

        Ok(())
    }

    #[tokio::test]
    async fn get_database() -> Result<(), Box<dyn std::error::Error>> {
        let api = test_client();

        let response = api
            .search(NotionSearch::Filter {
                value: FilterValue::Database,
                property: FilterProperty::Object,
            })
            .await?;

        let db = response.results()[0].clone();

        // todo: fix this clone issue
        let db_result = api.get_database(db.clone()).await?;

        assert_eq!(db, db_result);

        Ok(())
    }

    #[tokio::test]
    async fn query_database() -> Result<(), Box<dyn std::error::Error>> {
        let api = test_client();

        let response = api
            .search(NotionSearch::Filter {
                value: FilterValue::Database,
                property: FilterProperty::Object,
            })
            .await?;

        let db = dbg!(response.results()[0].clone());

        let pages = api
            .query_database(
                db,
                DatabaseQuery {
                    filter: Some(FilterCondition {
                        property: "Name".to_string(),
                        condition: Text(TextCondition::Contains("First".to_string())),
                    }),
                    ..Default::default()
                },
            )
            .await?;

        assert_eq!(pages.results().len(), 1);

        Ok(())
    }
}