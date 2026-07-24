use md5::{Digest, Md5};
use reqwest::multipart::{Form, Part};
use serde_json::Value;
use std::time::SystemTime;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{AttachmentSource, ThreadType};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UploadAttachmentType {
    pub file_type: String, // "image", "video", "others"
    pub finished: serde_json::Value,
    pub client_file_id: u64,
    pub chunk_id: u32,

    // Image fields
    pub photo_id: Option<String>,
    pub normal_url: Option<String>,
    pub hd_url: Option<String>,
    pub thumb_url: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub hd_size: Option<u64>,

    // Video & File fields
    pub file_url: Option<String>,
    pub file_id: Option<String>,
    pub checksum: Option<String>,
    pub total_size: Option<u64>,
    pub file_name: Option<String>,
}

fn get_timestamp_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn calculate_md5(data: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

impl Zalo {
    pub async fn upload_attachment(
        &self,
        sources: Vec<AttachmentSource>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Vec<UploadAttachmentType>, ZaloError> {
        if sources.is_empty() {
            return Err(ZaloError::PayloadFormat("Missing sources".to_string()));
        }

        // Limit checking
        let (max_file, max_size_bytes, max_size_mb, chunk_size, restricted_ext) = {
            let settings = self.ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
            let sharefile = settings.get("features").and_then(|f| f.get("sharefile"));

            let max_file = sharefile
                .and_then(|s| s.get("max_file"))
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize;
            let max_size_mb = sharefile
                .and_then(|s| s.get("max_size_share_file_v3"))
                .and_then(|v| v.as_u64())
                .unwrap_or(100);
            let max_size_bytes = max_size_mb * 1024 * 1024;

            let chunk_size = sharefile
                .and_then(|s| s.get("chunk_size_file"))
                .and_then(|v| v.as_u64())
                .unwrap_or(2 * 1024 * 1024) as usize;

            let restricted_ext: Vec<String> = sharefile
                .and_then(|s| s.get("restricted_ext_file"))
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(|s| s.to_lowercase()))
                        .collect()
                })
                .unwrap_or_default();
            (
                max_file,
                max_size_bytes,
                max_size_mb,
                chunk_size,
                restricted_ext,
            )
        };

        if sources.len() > max_file {
            return Err(ZaloError::ApiError {
                message: format!("Exceed maximum file of {}", max_file),
                code: None,
            });
        }

        let is_group = thread_type == ThreadType::Group;
        let endpoint_prefix = if is_group { "group" } else { "message" };
        let type_param = if is_group { "11" } else { "2" };

        let mut results = Vec::new();
        let mut client_id_seq = get_timestamp_ms();

        for source in sources {
            let (data, filename, metadata) = match source {
                AttachmentSource::Url(_) => {
                    return Err(ZaloError::PayloadFormat(
                        "URL attachment uploading not supported".to_string(),
                    ));
                }
                AttachmentSource::Buffer {
                    data,
                    filename,
                    metadata,
                } => (data, filename, metadata),
            };

            let ext = filename.split('.').next_back().unwrap_or("").to_lowercase();
            if restricted_ext.contains(&ext) {
                return Err(ZaloError::PayloadFormat(format!(
                    "File extension \"{}\" is not allowed",
                    ext
                )));
            }

            let total_size = data.len() as u64;
            if total_size > max_size_bytes {
                return Err(ZaloError::PayloadFormat(format!(
                    "File {} size exceeds maximum size of {}MB",
                    filename, max_size_mb
                )));
            }

            let file_type = match ext.as_str() {
                "jpg" | "jpeg" | "png" | "webp" => "image",
                "mp4" => "video",
                _ => "others",
            };

            let client_file_id = client_id_seq;
            client_id_seq += 1;

            let total_chunks = if total_size == 0 {
                1
            } else {
                ((total_size as f64) / (chunk_size as f64)).ceil() as u32
            };

            let mut final_result: Option<UploadAttachmentType> = None;

            // Prepare oneshot channel for video/others
            let mut rx = None;
            let mut file_id_for_ws: Option<String> = None;

            for chunk_idx in 0..total_chunks {
                let start = (chunk_idx as usize) * chunk_size;
                let end = std::cmp::min(start + chunk_size, data.len());
                let sliced_data = data.get(start..end).unwrap_or(&[]).to_vec();

                let part = Part::bytes(sliced_data)
                    .file_name(filename.clone())
                    .mime_str("application/octet-stream")?;

                let form = Form::new().part("chunkContent", part);

                let mut params = serde_json::json!({
                    "totalChunk": total_chunks,
                    "fileName": filename,
                    "clientId": client_file_id,
                    "totalSize": total_size,
                    "imei": self.ctx.imei,
                    "isE2EE": 0,
                    "jxl": 0,
                    "chunkId": chunk_idx + 1,
                });

                if is_group {
                    params
                        .as_object_mut()
                        .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
                } else {
                    params
                        .as_object_mut()
                        .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
                }

                let secret_key = self
                    .ctx
                    .get_secret_key()
                    .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
                let params_str = serde_json::to_string(&params)?;
                let encrypted_params =
                    crate::crypto::aes::encode_aes(&secret_key, params_str.as_bytes())?;

                let action_path = match file_type {
                    "image" => "photo_original/upload",
                    _ => "asyncfile/upload",
                };

                let path_url = format!("api/{}/{}", endpoint_prefix, action_path);
                let service_url = self.get_service_url("file", &path_url)?;
                let final_url = self.ctx.make_url(
                    &service_url,
                    &[("type", type_param), ("params", &encrypted_params)],
                    true,
                )?;

                let response = self
                    .ctx
                    .reqwest_client
                    .post(&final_url)
                    .header("Accept", "application/json, text/plain, */*")
                    .header("Origin", "https://chat.zalo.me")
                    .header("Referer", "https://chat.zalo.me/")
                    .multipart(form)
                    .send()
                    .await?;

                if !response.status().is_success() {
                    return Err(ZaloError::ApiError {
                        message: format!("HTTP error uploading chunk: {}", response.status()),
                        code: Some(response.status().as_u16() as i32),
                    });
                }

                let res_json: Value = response.json().await?;
                let error_code = res_json
                    .get("error_code")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(0);
                if error_code != 0 {
                    let msg = res_json
                        .get("error_message")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Upload error");
                    return Err(ZaloError::ApiError {
                        message: msg.to_string(),
                        code: Some(error_code as i32),
                    });
                }

                let data_val = res_json.get("data").ok_or_else(|| {
                    ZaloError::PayloadFormat("Upload response missing data".to_string())
                })?;

                let inner_data: Value = match data_val {
                    Value::String(encrypted_str) if !encrypted_str.trim().is_empty() => {
                        let secret_key = self
                            .ctx
                            .get_secret_key()
                            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
                        let decrypted_str = crate::crypto::aes::decode_aes(&secret_key, encrypted_str)?;
                        let decrypted_json: Value = serde_json::from_str(&decrypted_str).unwrap_or_default();
                        decrypted_json
                            .get("data")
                            .cloned()
                            .unwrap_or(decrypted_json)
                    }
                    _ => data_val.clone(),
                };

                let finished = inner_data
                    .get("finished")
                    .cloned()
                    .unwrap_or(Value::Bool(false));
                let chunk_id = inner_data
                    .get("chunkId")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(chunk_idx as u64 + 1) as u32;

                if file_type == "image" {
                    println!("Zalo upload photo decrypted inner_data: {:?}", inner_data);
                    let normal_url = inner_data
                        .get("normalUrl")
                        .or_else(|| inner_data.get("url"))
                        .or_else(|| inner_data.get("fileUrl"))
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => "".to_string(),
                        })
                        .unwrap_or_default();
                    let hd_url = inner_data
                        .get("hdUrl")
                        .or_else(|| inner_data.get("normalUrl"))
                        .or_else(|| inner_data.get("url"))
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => "".to_string(),
                        })
                        .unwrap_or_default();
                    let thumb_url = inner_data
                        .get("thumbUrl")
                        .or_else(|| inner_data.get("normalUrl"))
                        .or_else(|| inner_data.get("url"))
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => "".to_string(),
                        })
                        .unwrap_or_default();
                    let photo_id = inner_data
                        .get("photoId")
                        .or_else(|| inner_data.get("photo_id"))
                        .or_else(|| inner_data.get("id"))
                        .map(|v| match v {
                            Value::String(s) => s.clone(),
                            Value::Number(n) => n.to_string(),
                            _ => "".to_string(),
                        })
                        .unwrap_or_default();

                    // Calculate width/height from metadata or bytes
                    let mut width = metadata.width.unwrap_or(0);
                    let mut height = metadata.height.unwrap_or(0);
                    if width == 0 || height == 0 {
                        if let Some((w, h)) = get_image_dimensions_from_bytes(&data) {
                            width = w;
                            height = h;
                        }
                    }

                    final_result = Some(UploadAttachmentType {
                        file_type: "image".to_string(),
                        finished,
                        client_file_id,
                        chunk_id,
                        photo_id: Some(photo_id),
                        normal_url: Some(normal_url),
                        hd_url: Some(hd_url),
                        thumb_url: Some(thumb_url),
                        width: Some(width),
                        height: Some(height),
                        hd_size: Some(total_size),
                        file_url: None,
                        file_id: None,
                        checksum: None,
                        total_size: None,
                        file_name: None,
                    });
                } else {
                    let file_id = inner_data
                        .get("fileId")
                        .and_then(|v| v.as_str())
                        .unwrap_or_default()
                        .to_string();
                    if file_id != "-1" && !file_id.is_empty() {
                        file_id_for_ws = Some(file_id.clone());
                        let (tx, ws_rx) = tokio::sync::oneshot::channel();
                        rx = Some(ws_rx);
                        {
                            let mut callbacks = self
                                .ctx
                                .upload_callbacks
                                .lock()
                                .unwrap_or_else(|e| e.into_inner());
                            callbacks.insert(file_id, tx);
                        }
                    }
                }
            }

            // For videos and files, await WebSocket completion
            if file_type != "image" {
                if let (Some(ws_rx), Some(file_id)) = (rx, file_id_for_ws) {
                    let ws_data = ws_rx.await.map_err(|_err| {
                        ZaloError::Crypto(
                            "WebSocket upload notification channel closed".to_string(),
                        )
                    })?;
                    let checksum = calculate_md5(&data);
                    final_result = Some(UploadAttachmentType {
                        file_type: file_type.to_string(),
                        finished: Value::Bool(true),
                        client_file_id,
                        chunk_id: total_chunks,
                        photo_id: None,
                        normal_url: None,
                        hd_url: None,
                        thumb_url: None,
                        width: None,
                        height: None,
                        hd_size: None,
                        file_url: Some(ws_data.file_url),
                        file_id: Some(file_id),
                        checksum: Some(checksum),
                        total_size: Some(total_size),
                        file_name: Some(filename),
                    });
                } else {
                    return Err(ZaloError::ApiError {
                        message: "Failed to upload file chunk: missing fileId".to_string(),
                        code: None,
                    });
                }
            }

            if let Some(res) = final_result {
                results.push(res);
            }
        }

        Ok(results)
    }

    pub async fn upload_product_photo(
        &self,
        source: &AttachmentSource,
    ) -> Result<UploadAttachmentType, ZaloError> {
        let (data, _filename, metadata) = match source {
            AttachmentSource::Url(_) => {
                return Err(ZaloError::PayloadFormat(
                    "URL source not supported".to_string(),
                ));
            }
            AttachmentSource::Buffer {
                data,
                filename,
                metadata,
            } => (data, filename, metadata),
        };

        let file_size = data.len() as u64;
        let client_file_id = get_timestamp_ms();

        let part = Part::bytes(data.clone())
            .file_name("undefined")
            .mime_str("application/octet-stream")?;
        let form = Form::new().part("chunkContent", part);

        let send2me_id = {
            let login_info = self
                .ctx
                .login_info
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            login_info
                .get("send2me_id")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string()
        };

        let params = serde_json::json!({
            "totalChunk": 1,
            "fileName": format!("Base64_Img_Picker_{}.jpg", get_timestamp_ms()),
            "clientId": client_file_id,
            "totalSize": file_size,
            "imei": self.ctx.imei,
            "chunkId": 1,
            "toid": send2me_id,
            "featureId": 1,
        });

        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let params_str = serde_json::to_string(&params)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, params_str.as_bytes())?;

        let service_url = self.get_service_url("file", "api/product/upload/photo")?;
        let final_url = self
            .ctx
            .make_url(&service_url, &[("params", &encrypted_params)], true)?;

        let response = self
            .ctx
            .reqwest_client
            .post(&final_url)
            .header("Accept", "application/json, text/plain, */*")
            .header("Origin", "https://chat.zalo.me")
            .header("Referer", "https://chat.zalo.me/")
            .multipart(form)
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(ZaloError::ApiError {
                message: format!("HTTP error uploading product photo: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let res_json: Value = response.json().await?;
        let error_code = res_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = res_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Upload product photo error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let inner_data = res_json.get("data").ok_or_else(|| {
            ZaloError::PayloadFormat("Product photo response missing data".to_string())
        })?;
        let normal_url = inner_data
            .get("normalUrl")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let hd_url = inner_data
            .get("hdUrl")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let thumb_url = inner_data
            .get("thumbUrl")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let photo_id = inner_data
            .get("photoId")
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string();
        let finished = inner_data
            .get("finished")
            .cloned()
            .unwrap_or(Value::Bool(false));

        let mut width = metadata.width.unwrap_or(0);
        let mut height = metadata.height.unwrap_or(0);
        if width == 0 || height == 0 {
            if let Some((w, h)) = get_image_dimensions_from_bytes(data) {
                width = w;
                height = h;
            }
        }

        Ok(UploadAttachmentType {
            file_type: "image".to_string(),
            finished,
            client_file_id,
            chunk_id: 1,
            photo_id: Some(photo_id),
            normal_url: Some(normal_url),
            hd_url: Some(hd_url),
            thumb_url: Some(thumb_url),
            width: Some(width),
            height: Some(height),
            hd_size: Some(file_size),
            file_url: None,
            file_id: None,
            checksum: None,
            total_size: None,
            file_name: None,
        })
    }
}

pub(crate) fn get_image_dimensions_from_bytes(data: &[u8]) -> Option<(u32, u32)> {
    #[cfg(feature = "image-metadata")]
    {
        if let Ok(img) = image::load_from_memory(data) {
            use image::GenericImageView;
            return Some(img.dimensions());
        }
    }
    let _ = data;
    None
}
