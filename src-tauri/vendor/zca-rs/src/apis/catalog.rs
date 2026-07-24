//! Catalog Submodule for Zalo API
//!
//! Provides APIs to manage catalogs and product catalogs for zBusiness accounts.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{AttachmentSource, CatalogItem, ProductCatalogItem};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateCatalogResponse {
    pub item: CatalogItem,
    pub version_ls_catalog: u32,
    pub version_catalog: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateProductCatalogPayload {
    pub catalog_id: String,
    pub product_name: String,
    pub price: String,
    pub description: String,
    pub files: Option<Vec<AttachmentSource>>,
    pub product_photos: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProductCatalogResponse {
    pub item: ProductCatalogItem,
    pub version_ls_catalog: u32,
    pub version_catalog: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateCatalogPayload {
    pub catalog_id: String,
    pub catalog_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateCatalogResponse {
    pub item: CatalogItem,
    pub version_ls_catalog: u32,
    pub version_catalog: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProductCatalogPayload {
    pub catalog_id: String,
    pub product_id: String,
    pub product_name: String,
    pub price: String,
    pub description: String,
    pub create_time: u64,
    pub files: Option<Vec<AttachmentSource>>,
    pub product_photos: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProductCatalogResponse {
    pub item: ProductCatalogItem,
    pub version_ls_catalog: u32,
    pub version_catalog: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteProductCatalogPayload {
    pub product_ids: Vec<String>,
    pub catalog_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteProductCatalogResponse {
    pub item: Vec<u64>,
    pub version_ls_catalog: u32,
    pub version_catalog: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetCatalogListPayload {
    pub limit: Option<u32>,
    pub last_product_id: Option<i64>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetCatalogListResponse {
    pub items: Vec<CatalogItem>,
    pub version: u32,
    pub has_more: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetProductCatalogListPayload {
    pub catalog_id: String,
    pub limit: Option<u32>,
    pub version_catalog: Option<u32>,
    pub last_product_id: Option<String>,
    pub page: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetProductCatalogListResponse {
    pub items: Vec<ProductCatalogItem>,
    pub version: u32,
    pub has_more: u32,
}

impl Zalo {
    /// Create a new catalog
    pub async fn create_catalog(
        &self,
        catalog_name: &str,
    ) -> Result<CreateCatalogResponse, ZaloError> {
        let params = serde_json::json!({
            "catalog_name": catalog_name,
            "catalog_photo": "",
        });

        self.request_api_form("catalog", "api/prodcatalog/catalog/create", params)
            .await
    }

    /// Create a product catalog item, optionally uploading attached photo files
    pub async fn create_product_catalog(
        &self,
        payload: CreateProductCatalogPayload,
    ) -> Result<CreateProductCatalogResponse, ZaloError> {
        let mut product_photo = payload.product_photos.unwrap_or_default();

        if let Some(files) = &payload.files {
            if !files.is_empty() {
                if files.len() > 5 {
                    return Err(ZaloError::PayloadFormat(
                        "Maximum 5 media files are allowed".to_string(),
                    ));
                }
                for media_file in files {
                    let upload_media = self.upload_product_photo(media_file).await?;
                    let url = upload_media
                        .normal_url
                        .or(upload_media.hd_url)
                        .unwrap_or_default();
                    product_photo.push(url);
                }
            }
        }

        if product_photo.len() > 5 {
            return Err(ZaloError::PayloadFormat(
                "Maximum 5 media files are allowed".to_string(),
            ));
        }

        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let params = serde_json::json!({
            "product_name": payload.product_name,
            "price": payload.price,
            "description": payload.description,
            "product_photos": product_photo,
            "catalog_id": payload.catalog_id,
            "currency_unit": "₫",
            "create_time": now_ms,
        });

        self.request_api_form("catalog", "api/prodcatalog/product/create", params)
            .await
    }

    /// Update an existing catalog
    pub async fn update_catalog(
        &self,
        payload: UpdateCatalogPayload,
    ) -> Result<UpdateCatalogResponse, ZaloError> {
        let params = serde_json::json!({
            "catalog_id": payload.catalog_id,
            "catalog_name": payload.catalog_name,
            "catalog_photo": "",
        });

        self.request_api_form("catalog", "api/prodcatalog/catalog/update", params)
            .await
    }

    /// Update an existing product catalog item, optionally uploading attached photo files
    pub async fn update_product_catalog(
        &self,
        payload: UpdateProductCatalogPayload,
    ) -> Result<UpdateProductCatalogResponse, ZaloError> {
        let mut product_photo = payload.product_photos.unwrap_or_default();

        if let Some(files) = &payload.files {
            if !files.is_empty() {
                if files.len() > 5 {
                    return Err(ZaloError::PayloadFormat(
                        "Maximum 5 media files are allowed".to_string(),
                    ));
                }
                for media_file in files {
                    let upload_media = self.upload_product_photo(media_file).await?;
                    let url = upload_media
                        .normal_url
                        .or(upload_media.hd_url)
                        .unwrap_or_default();
                    product_photo.push(url);
                }
            }
        }

        if product_photo.len() > 5 {
            return Err(ZaloError::PayloadFormat(
                "Maximum 5 media files are allowed".to_string(),
            ));
        }

        let params = serde_json::json!({
            "product_id": payload.product_id,
            "product_name": payload.product_name,
            "price": payload.price,
            "description": payload.description,
            "product_photos": product_photo,
            "catalog_id": payload.catalog_id,
            "currency_unit": "₫",
            "create_time": payload.create_time,
        });

        self.request_api_form("catalog", "api/prodcatalog/product/update", params)
            .await
    }

    /// Delete a catalog by ID
    pub async fn delete_catalog(&self, catalog_id: &str) -> Result<Value, ZaloError> {
        let params = serde_json::json!({
            "catalog_id": catalog_id,
        });

        self.request_api_form("catalog", "api/prodcatalog/catalog/delete", params)
            .await
    }

    /// Delete multiple product catalog items from a catalog
    pub async fn delete_product_catalog(
        &self,
        payload: DeleteProductCatalogPayload,
    ) -> Result<DeleteProductCatalogResponse, ZaloError> {
        let params = serde_json::json!({
            "product_ids": payload.product_ids,
            "catalog_id": payload.catalog_id,
        });

        self.request_api_form("catalog", "api/prodcatalog/product/mdelete", params)
            .await
    }

    /// Retrieve the list of catalogs
    pub async fn get_catalog_list(
        &self,
        payload: Option<GetCatalogListPayload>,
    ) -> Result<GetCatalogListResponse, ZaloError> {
        let payload = payload.unwrap_or(GetCatalogListPayload {
            limit: None,
            last_product_id: None,
            page: None,
        });

        let params = serde_json::json!({
            "version_list_catalog": 0,
            "limit": payload.limit.unwrap_or(20),
            "last_product_id": payload.last_product_id.unwrap_or(-1),
            "page": payload.page.unwrap_or(0),
        });

        self.request_api_form("catalog", "api/prodcatalog/catalog/list", params)
            .await
    }

    /// Retrieve the list of product catalog items for a given catalog
    pub async fn get_product_catalog_list(
        &self,
        payload: GetProductCatalogListPayload,
    ) -> Result<GetProductCatalogListResponse, ZaloError> {
        let last_prod_id = match &payload.last_product_id {
            Some(s) => serde_json::json!(s),
            None => serde_json::json!(-1),
        };

        let params = serde_json::json!({
            "catalog_id": payload.catalog_id,
            "limit": payload.limit.unwrap_or(100),
            "version_catalog": payload.version_catalog.unwrap_or(0),
            "last_product_id": last_prod_id,
            "page": payload.page.unwrap_or(0),
        });

        self.request_api_form("catalog", "api/prodcatalog/product/list", params)
            .await
    }
}
