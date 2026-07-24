use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{StickerBasic, StickerDetail};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StickerSuggestions {
    pub sugg_sticker: Option<Vec<StickerBasic>>,
    pub sugg_guggy: Option<Vec<StickerBasic>>,
    pub sugg_gif: Option<Vec<StickerBasic>>,
}

impl Zalo {
    /// Get stickers by keyword
    pub async fn get_stickers(&self, keyword: &str) -> Result<Vec<u32>, ZaloError> {
        if keyword.is_empty() {
            return Err(ZaloError::PayloadFormat("Missing keyword".to_string()));
        }

        let params = serde_json::json!({
            "keyword": keyword,
            "gif": 1,
            "guggy": 0,
            "imei": self.ctx.imei,
        });

        let suggestions: StickerSuggestions = self
            .request_api(
                reqwest::Method::GET,
                "sticker",
                "api/message/sticker/suggest/stickers",
                params,
                true,
            )
            .await?;

        let mut sticker_ids = Vec::new();
        if let Some(sugg) = suggestions.sugg_sticker {
            for sticker in sugg {
                sticker_ids.push(sticker.sticker_id);
            }
        }

        Ok(sticker_ids)
    }

    /// Get stickers detail by sticker IDs
    pub async fn get_stickers_detail(
        &self,
        sticker_ids: &[u32],
    ) -> Result<Vec<StickerDetail>, ZaloError> {
        if sticker_ids.is_empty() {
            return Err(ZaloError::PayloadFormat("Missing sticker id".to_string()));
        }

        let mut futures = Vec::new();
        for &sticker_id in sticker_ids {
            futures.push(self.get_sticker_detail(sticker_id));
        }

        let results = futures_util::future::join_all(futures).await;
        let mut stickers = Vec::new();
        for detail in results.into_iter().flatten() {
            stickers.push(detail);
        }

        Ok(stickers)
    }

    async fn get_sticker_detail(&self, sticker_id: u32) -> Result<StickerDetail, ZaloError> {
        let params = serde_json::json!({
            "sid": sticker_id,
        });

        self.request_api(
            reqwest::Method::GET,
            "sticker",
            "api/message/sticker/sticker_detail",
            params,
            true,
        )
        .await
    }

    /// Search stickers
    pub async fn search_sticker(
        &self,
        keyword: &str,
        limit: Option<u32>,
    ) -> Result<Vec<StickerBasic>, ZaloError> {
        let params = serde_json::json!({
            "keyword": keyword,
            "limit": limit.unwrap_or(50),
            "srcType": 0,
            "imei": self.ctx.imei,
        });

        self.request_api(
            reqwest::Method::GET,
            "sticker",
            "api/message/sticker/search",
            params,
            true,
        )
        .await
    }

    /// Get sticker category detail
    pub async fn get_sticker_category_detail(
        &self,
        cate_id: u32,
    ) -> Result<Vec<StickerDetail>, ZaloError> {
        let params = serde_json::json!({
            "cid": cate_id,
        });

        self.request_api(
            reqwest::Method::GET,
            "sticker",
            "api/message/sticker/category/sticker_detail",
            params,
            true,
        )
        .await
    }
}
