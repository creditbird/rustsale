use reqwest::multipart::{Form, Part};
use serde_json::Value;

use crate::apis::Zalo;
use crate::error::ZaloError;
use crate::models::{
    AttachmentSource, MessageContent, MessageMention, MessageStyle, Reactions, SendMessageQuote,
    SendMessageResponse, SendMessageResult, ThreadType, Urgency,
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum ReactionType {
    Standard(Reactions),
    Custom {
        r_type: i32,
        source: i32,
        icon: String,
    },
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeenMessageParams {
    pub msg_id: String,
    pub cli_msg_id: String,
    pub uid_from: String,
    pub id_to: String,
    pub msg_type: String,
    pub st: i32,
    pub at: i32,
    pub cmd: i32,
    pub ts: serde_json::Value,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ForwardReference {
    pub id: String,
    pub ts: u64,
    pub log_src_type: i32,
    pub fw_lvl: i32,
}

fn get_timestamp_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn calculate_md5(data: &[u8]) -> String {
    use md5::Digest;
    let mut hasher = md5::Md5::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

fn get_file_extension(filename: &str) -> String {
    filename.split('.').next_back().unwrap_or("").to_lowercase()
}

fn get_attachment_filename(source: &AttachmentSource) -> String {
    match source {
        AttachmentSource::Url(url) => url.split('/').next_back().unwrap_or("").to_string(),
        AttachmentSource::Buffer { filename, .. } => filename.clone(),
    }
}

fn get_image_dimensions_from_bytes(data: &[u8]) -> Option<(u32, u32)> {
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

impl Zalo {
    pub async fn send_message(
        &self,
        message: MessageContent,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<SendMessageResponse, ZaloError> {
        self.send_message_with_client_id(message, thread_id, thread_type, get_timestamp_ms())
            .await
    }

    pub async fn send_message_with_client_id(
        &self,
        message: MessageContent,
        thread_id: &str,
        thread_type: ThreadType,
        client_id: u64,
    ) -> Result<SendMessageResponse, ZaloError> {
        let mut msg = message.msg.clone();
        let attachments = message.attachments.clone().unwrap_or_default();
        let mentions = message.mentions.clone();
        let quote = message.quote.clone();
        let ttl = message.ttl;
        let styles = message.styles.clone();
        let urgency = message.urgency;

        if attachments.len() > 1 {
            return Err(ZaloError::PayloadFormat(
                "send_message_with_client_id accepts at most 1 attachment per part".to_string(),
            ));
        }

        if msg.is_empty() && attachments.is_empty() {
            return Err(ZaloError::PayloadFormat(
                "Missing message content".to_string(),
            ));
        }

        let max_file = {
            let settings = self.ctx.settings.lock().unwrap_or_else(|e| e.into_inner());
            let sharefile = settings.get("features").and_then(|f| f.get("sharefile"));
            sharefile
                .and_then(|s| s.get("max_file"))
                .and_then(|v| v.as_u64())
                .unwrap_or(50) as usize
        };
        if attachments.len() > max_file {
            return Err(ZaloError::ApiError {
                message: format!("Exceed maximum file of {}", max_file),
                code: None,
            });
        }

        let mut responses = SendMessageResponse {
            message: None,
            attachment: Vec::new(),
        };

        if !attachments.is_empty() {
            let first_att = attachments
                .first()
                .ok_or_else(|| ZaloError::PayloadFormat("Missing attachments".to_string()))?;
            let first_filename = get_attachment_filename(first_att);
            let first_ext = get_file_extension(&first_filename);
            let is_single_file = attachments.len() == 1;
            let can_be_desc =
                is_single_file && matches!(first_ext.as_str(), "jpg" | "jpeg" | "png" | "webp");

            if (!can_be_desc && !msg.is_empty()) || (!msg.is_empty() && quote.is_some()) {
                let text_msg_content = MessageContent {
                    msg: msg.clone(),
                    styles: styles.clone(),
                    urgency,
                    quote: quote.clone(),
                    mentions: mentions.clone(),
                    attachments: None,
                    ttl,
                };
                let msg_result = self
                    .send_text_only_message_with_client_id(text_msg_content, thread_id, thread_type, client_id)
                    .await?;
                responses.message = Some(msg_result);
                msg = String::new();
            }

            let mut attachment_results = self
                .send_attachments_helper(
                    &msg,
                    attachments.clone(),
                    mentions.clone(),
                    quote.clone(),
                    ttl,
                    styles.clone(),
                    urgency,
                    thread_id,
                    thread_type,
                )
                .await?;
            for r in &mut attachment_results {
                if r.cli_msg_id == 0 {
                    r.cli_msg_id = client_id;
                }
            }
            responses.attachment = attachment_results;
            msg = String::new();
        }

        if !msg.is_empty() {
            let text_msg_content = MessageContent {
                msg,
                styles,
                urgency,
                quote,
                mentions,
                attachments: None,
                ttl,
            };
            let msg_result = self
                .send_text_only_message_with_client_id(text_msg_content, thread_id, thread_type, client_id)
                .await?;
            responses.message = Some(msg_result);
        }

        Ok(responses)
    }

    async fn send_text_only_message_with_client_id(
        &self,
        message: MessageContent,
        thread_id: &str,
        thread_type: ThreadType,
        client_id: u64,
    ) -> Result<SendMessageResult, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let mut params = build_text_params(
            &message.msg,
            thread_id,
            thread_type,
            client_id,
            message.quote.as_ref(),
            message.mentions.as_deref(),
            message.ttl,
            &self.ctx.imei,
        );

        let has_mentions = is_group
            && message
                .mentions
                .as_ref()
                .map(|m| m.iter().any(|item| item.pos >= 0 && !item.uid.is_empty() && item.len > 0))
                .unwrap_or(false);

        if let Some(st_list) = &message.styles {
            let mut formatted_styles = Vec::new();
            for s in st_list {
                formatted_styles.push(serde_json::json!({
                    "start": s.start,
                    "len": s.len,
                    "st": s.st,
                }));
            }
            let text_props = serde_json::json!({
                "styles": formatted_styles,
                "ver": 0,
            });
            params.as_object_mut().map(|m| {
                m.insert(
                    "textProperties".to_string(),
                    serde_json::json!(text_props.to_string()),
                )
            });
        }

        if let Some(urg) = message.urgency {
            if urg == Urgency::Important || urg == Urgency::Urgent {
                params.as_object_mut().map(|m| {
                    m.insert(
                        "metaData".to_string(),
                        serde_json::json!({ "urgency": urg as u8 }),
                    )
                });
            }
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group { "api/group" } else { "api/message" };
        let path_buf = if message.quote.is_some() {
            format!("{}/quote", path)
        } else {
            let suffix = if is_group {
                if has_mentions {
                    "mention"
                } else {
                    "sendmsg"
                }
            } else {
                "sms"
            };
            format!("{}/{}", path, suffix)
        };

        let mut res: SendMessageResult = self
            .request_api_form_with_query(service, &path_buf, &[("nretry", "0")], params)
            .await?;
        if res.cli_msg_id == 0 {
            res.cli_msg_id = client_id;
        }
        Ok(res)
    }

    #[expect(clippy::too_many_arguments)]
    async fn send_attachments_helper(
        &self,
        msg: &str,
        attachments: Vec<AttachmentSource>,
        mentions: Option<Vec<MessageMention>>,
        quote: Option<SendMessageQuote>,
        ttl: Option<u64>,
        _styles: Option<Vec<MessageStyle>>,
        urgency: Option<Urgency>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Vec<SendMessageResult>, ZaloError> {
        let is_group = thread_type == ThreadType::Group;

        let mut gifs = Vec::new();
        let mut others = Vec::new();
        for a in attachments {
            let filename = get_attachment_filename(&a);
            let ext = get_file_extension(&filename);
            if ext == "gif" {
                gifs.push(a);
            } else {
                others.push(a);
            }
        }

        let upload_attachments = if others.is_empty() {
            Vec::new()
        } else {
            self.upload_attachment(others, thread_id, thread_type)
                .await?
        };

        let mut results = Vec::new();
        let mut client_id_seq = get_timestamp_ms();

        let is_multi_file = upload_attachments.len() > 1;
        let group_layout_id = get_timestamp_ms().to_string();
        let mut index_in_group = 0;

        let mentions_val = if is_group && upload_attachments.len() == 1 {
            if let Some(m_list) = &mentions {
                let filtered: Vec<serde_json::Value> = m_list
                    .iter()
                    .filter(|m| m.pos >= 0 && !m.uid.is_empty() && m.len > 0)
                    .map(|m| {
                        serde_json::json!({
                            "pos": m.pos,
                            "uid": m.uid,
                            "len": m.len,
                            "type": if m.uid == "-1" { 1 } else { 0 },
                        })
                    })
                    .collect();
                if filtered.is_empty() {
                    serde_json::Value::Null
                } else {
                    serde_json::json!(filtered)
                }
            } else {
                serde_json::Value::Null
            }
        } else {
            serde_json::Value::Null
        };

        let can_be_desc = upload_attachments.len() == 1;

        for att in upload_attachments {
            let mut params = serde_json::json!({
                "ttl": ttl.unwrap_or(0),
            });

            if let Some(urg) = urgency {
                if urg == Urgency::Important || urg == Urgency::Urgent {
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "metaData".to_string(),
                            serde_json::json!({ "urgency": urg as u8 }),
                        )
                    });
                }
            }

            match att.file_type.as_str() {
                "image" => {
                    let photo_id_val = att.photo_id.clone().unwrap_or_default();
                    params
                        .as_object_mut()
                        .map(|m| m.insert("photoId".to_string(), serde_json::json!(photo_id_val)));
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "clientId".to_string(),
                            serde_json::json!((client_id_seq).to_string()),
                        )
                    });
                    client_id_seq += 1;
                    params
                        .as_object_mut()
                        .map(|m| m.insert("desc".to_string(), serde_json::json!(msg)));
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "width".to_string(),
                            serde_json::json!(att.width.unwrap_or(0)),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "height".to_string(),
                            serde_json::json!(att.height.unwrap_or(0)),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "rawUrl".to_string(),
                            serde_json::json!(att.normal_url.clone().unwrap_or_default()),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "hdUrl".to_string(),
                            serde_json::json!(att.hd_url.clone().unwrap_or_default()),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "thumbUrl".to_string(),
                            serde_json::json!(att.thumb_url.clone().unwrap_or_default()),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "hdSize".to_string(),
                            serde_json::json!(att.hd_size.unwrap_or(0).to_string()),
                        )
                    });
                    params
                        .as_object_mut()
                        .map(|m| m.insert("zsource".to_string(), serde_json::json!(-1)));
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "jcp".to_string(),
                            serde_json::json!("{\"convertible\":\"jxl\"}"),
                        )
                    });

                    if is_group {
                        params
                            .as_object_mut()
                            .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "oriUrl".to_string(),
                                serde_json::json!(att.normal_url.clone().unwrap_or_default()),
                            )
                        });
                    } else {
                        params
                            .as_object_mut()
                            .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "normalUrl".to_string(),
                                serde_json::json!(att.normal_url.clone().unwrap_or_default()),
                            )
                        });
                    }

                    if is_multi_file {
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "groupLayoutId".to_string(),
                                serde_json::json!(group_layout_id),
                            )
                        });
                        params
                            .as_object_mut()
                            .map(|m| m.insert("isGroupLayout".to_string(), serde_json::json!(1)));
                        params.as_object_mut().map(|m| {
                            m.insert("idInGroup".to_string(), serde_json::json!(index_in_group))
                        });
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "totalItemInGroup".to_string(),
                                serde_json::json!(att.chunk_id),
                            )
                        }); // chunk_id was total file count in JS
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "extMsgProp".to_string(),
                                serde_json::json!(format!(
                                    "{{\"groupMediaMsg\":{{\"groupLayoutId\":\"{}\"}}}}",
                                    group_layout_id
                                )),
                            )
                        });
                        index_in_group += 1;
                    }

                    if !mentions_val.is_null() && can_be_desc && quote.is_none() {
                        params.as_object_mut().map(|m| {
                            m.insert(
                                "mentionInfo".to_string(),
                                serde_json::json!(mentions_val.to_string()),
                            )
                        });
                    }
                }
                "video" | "others" => {
                    params
                        .as_object_mut()
                        .map(|m| m.insert("fileId".to_string(), serde_json::json!(att.file_id)));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("checksum".to_string(), serde_json::json!(att.checksum)));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("checksumSha".to_string(), serde_json::json!("")));
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "extention".to_string(),
                            serde_json::json!(get_file_extension(
                                &att.file_name.clone().unwrap_or_default()
                            )),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "totalSize".to_string(),
                            serde_json::json!(att.total_size.unwrap_or(0)),
                        )
                    });
                    params.as_object_mut().map(|m| {
                        m.insert("fileName".to_string(), serde_json::json!(att.file_name))
                    });
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "clientId".to_string(),
                            serde_json::json!(att.client_file_id.to_string()),
                        )
                    });
                    params
                        .as_object_mut()
                        .map(|m| m.insert("fType".to_string(), serde_json::json!(1)));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("fileCount".to_string(), serde_json::json!(0)));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("fdata".to_string(), serde_json::json!("{}")));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("zsource".to_string(), serde_json::json!(-1)));
                    params
                        .as_object_mut()
                        .map(|m| m.insert("fileUrl".to_string(), serde_json::json!(att.file_url)));

                    if is_group {
                        params
                            .as_object_mut()
                            .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
                    } else {
                        params
                            .as_object_mut()
                            .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
                    }
                }
                _ => {}
            }

            let res = self
                .send_attachment_msg(&att.file_type, params, thread_type)
                .await?;
            results.push(res);
        }

        for gif in gifs {
            let (data, filename) = match gif {
                AttachmentSource::Url(_) => {
                    return Err(ZaloError::PayloadFormat(
                        "URL sources not supported for GIF".to_string(),
                    ));
                }
                AttachmentSource::Buffer { data, filename, .. } => (data, filename),
            };

            let md5_val = calculate_md5(&data);
            let upthumb_res = self.up_thumb(&data).await?;
            let thumb_url = upthumb_res
                .get("url")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string();

            let mut width = 200;
            let mut height = 200;
            if let Some((w, h)) = get_image_dimensions_from_bytes(&data) {
                width = w;
                height = h;
            }

            let mut params = serde_json::json!({
                "clientId": get_timestamp_ms().to_string(),
                "fileName": filename,
                "totalSize": data.len(),
                "width": width,
                "height": height,
                "msg": msg,
                "type": 1,
                "ttl": ttl.unwrap_or(0),
                "thumb": thumb_url,
                "checksum": md5_val,
                "totalChunk": 1,
                "chunkId": 1,
            });

            if is_group {
                params
                    .as_object_mut()
                    .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
                params
                    .as_object_mut()
                    .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
            } else {
                params
                    .as_object_mut()
                    .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
            }

            if let Some(urg) = urgency {
                if urg == Urgency::Important || urg == Urgency::Urgent {
                    params.as_object_mut().map(|m| {
                        m.insert(
                            "metaData".to_string(),
                            serde_json::json!({ "urgency": urg as u8 }),
                        )
                    });
                }
            }

            let res_val = self
                .send_gif_chunk(&data, &filename, params, thread_type)
                .await?;
            let msg_id = res_val.get("msgId").and_then(|v| v.as_u64()).unwrap_or(0);
            let cli_msg_id = res_val.get("cliMsgId").and_then(|v| v.as_u64()).unwrap_or(0);
            results.push(SendMessageResult { msg_id, cli_msg_id });
        }

        Ok(results)
    }

    async fn send_attachment_msg(
        &self,
        file_type: &str,
        params_val: serde_json::Value,
        thread_type: ThreadType,
    ) -> Result<SendMessageResult, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let endpoint_prefix = if is_group { "group" } else { "message" };

        let sub_path = match file_type {
            "image" => "photo_original/send",
            _ => "asyncfile/msg",
        };

        let action_path = format!("api/{}/{}", endpoint_prefix, sub_path);
        self.request_api_form_with_query("file", &action_path, &[("nretry", "0")], params_val)
            .await
    }

    async fn up_thumb(&self, data: &[u8]) -> Result<serde_json::Value, ZaloError> {
        let client_id = get_timestamp_ms();
        let params = serde_json::json!({
            "clientId": client_id,
            "imei": self.ctx.imei,
        });
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let params_str = serde_json::to_string(&params)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, params_str.as_bytes())?;

        let service_url = self.get_service_url("file", "api/message/upthumb")?;
        let final_url = self
            .ctx
            .make_url(&service_url, &[("params", &encrypted_params)], true)?;

        let part = Part::bytes(data.to_vec())
            .file_name("blob")
            .mime_str("image/png")?;
        let form = Form::new().part("fileContent", part);

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
                message: format!("HTTP error upthumb: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let res_json: serde_json::Value = response.json().await?;
        let error_code = res_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = res_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Upthumb error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let inner_data = res_json
            .get("data")
            .ok_or_else(|| ZaloError::PayloadFormat("Upthumb missing data".to_string()))?;
        Ok(inner_data.clone())
    }

    async fn send_gif_chunk(
        &self,
        gif_data: &[u8],
        filename: &str,
        params_val: serde_json::Value,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let secret_key = self
            .ctx
            .get_secret_key()
            .ok_or_else(|| ZaloError::Crypto("Context missing secret_key".to_string()))?;
        let params_str = serde_json::to_string(&params_val)?;
        let encrypted_params = crate::crypto::aes::encode_aes(&secret_key, params_str.as_bytes())?;

        let is_group = thread_type == ThreadType::Group;
        let endpoint_prefix = if is_group { "group" } else { "message" };
        let action_path = format!("api/{}/gif", endpoint_prefix);

        let service_url = self.get_service_url("file", &action_path)?;
        let final_url = self.ctx.make_url(
            &service_url,
            &[
                ("params", &encrypted_params),
                ("type", "1"),
                ("nretry", "0"),
            ],
            true,
        )?;

        let part = Part::bytes(gif_data.to_vec())
            .file_name(filename.to_string())
            .mime_str("application/octet-stream")?;
        let form = Form::new().part("chunkContent", part);

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
                message: format!("HTTP error sending gif: {}", response.status()),
                code: Some(response.status().as_u16() as i32),
            });
        }

        let res_json: serde_json::Value = response.json().await?;
        let error_code = res_json
            .get("error_code")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        if error_code != 0 {
            let msg = res_json
                .get("error_message")
                .and_then(|v| v.as_str())
                .unwrap_or("Send gif error");
            return Err(ZaloError::ApiError {
                message: msg.to_string(),
                code: Some(error_code as i32),
            });
        }

        let inner_data = res_json
            .get("data")
            .ok_or_else(|| ZaloError::PayloadFormat("Send gif missing data".to_string()))?;
        Ok(inner_data.clone())
    }

    pub async fn send_sticker(
        &self,
        sticker_id: u32,
        cate_id: u32,
        sticker_type: u32,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<SendMessageResult, ZaloError> {
        self.send_sticker_with_client_id(
            sticker_id,
            cate_id,
            sticker_type,
            thread_id,
            thread_type,
            get_timestamp_ms(),
        )
        .await
    }

    pub async fn send_sticker_with_client_id(
        &self,
        sticker_id: u32,
        cate_id: u32,
        sticker_type: u32,
        thread_id: &str,
        thread_type: ThreadType,
        client_id: u64,
    ) -> Result<SendMessageResult, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let params = build_sticker_params(
            sticker_id,
            cate_id,
            sticker_type,
            thread_id,
            thread_type,
            client_id,
            &self.ctx.imei,
        );

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/sticker"
        } else {
            "api/message/sticker"
        };

        let mut res: SendMessageResult = self
            .request_api_form_with_query(service, path, &[("nretry", "0")], params)
            .await?;
        if res.cli_msg_id == 0 {
            res.cli_msg_id = client_id;
        }
        Ok(res)
    }

    pub async fn add_reaction(
        &self,
        icon: ReactionType,
        msg_id: &str,
        cli_msg_id: &str,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<Vec<u64>, ZaloError> {
        let (r_type, source, r_icon) = match icon {
            ReactionType::Standard(rec) => {
                let (t, s) = rec.to_type_and_source();
                (t, s, rec.to_str().to_string())
            }
            ReactionType::Custom {
                r_type,
                source,
                icon,
            } => (r_type, source, icon),
        };

        let is_group = thread_type == ThreadType::Group;

        let msg_obj = serde_json::json!({
            "gMsgID": msg_id.parse::<u64>().unwrap_or(0),
            "cMsgID": cli_msg_id.parse::<u64>().unwrap_or(0),
            "msgType": 1,
        });

        let react_msg = serde_json::json!({
            "rMsg": [msg_obj],
            "rIcon": r_icon,
            "rType": r_type,
            "source": source,
        });

        let react_item = serde_json::json!({
            "message": serde_json::to_string(&react_msg)?,
            "clientId": get_timestamp_ms(),
        });

        let mut params = serde_json::json!({
            "react_list": [react_item],
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
        }

        let service = "reaction";
        let path = if is_group {
            "api/group/reaction"
        } else {
            "api/message/reaction"
        };

        let res: Value = self.request_api_form(service, path, params).await?;

        if let Some(msg_ids_val) = res.get("msgIds") {
            if let Some(s) = msg_ids_val.as_str() {
                let parsed: Vec<u64> = serde_json::from_str(s)?;
                return Ok(parsed);
            }
            if let Some(arr) = msg_ids_val.as_array() {
                let parsed: Vec<u64> = arr.iter().filter_map(|v| v.as_u64()).collect();
                return Ok(parsed);
            }
        }

        Ok(Vec::new())
    }

    pub async fn undo(
        &self,
        msg_id: &str,
        cli_msg_id: &str,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "msgId": msg_id.parse::<u64>().unwrap_or(0),
            "clientId": get_timestamp_ms(),
            "cliMsgIdUndo": cli_msg_id.parse::<u64>().unwrap_or(0),
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/undomsg"
        } else {
            "api/message/undo"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn delete_message(
        &self,
        msg_id: &str,
        cli_msg_id: &str,
        uid_from: &str,
        thread_id: &str,
        thread_type: ThreadType,
        only_me: bool,
    ) -> Result<serde_json::Value, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let is_self = {
            let uid_guard = self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            uid_guard.as_deref() == Some(uid_from)
        };

        if is_self && !only_me {
            return Err(ZaloError::PayloadFormat(
                "To delete your message for everyone, use undo api instead".to_string(),
            ));
        }

        if !is_group && !only_me {
            return Err(ZaloError::PayloadFormat(
                "Can't delete message for everyone in a private chat".to_string(),
            ));
        }

        let mut params = serde_json::json!({
            "cliMsgId": get_timestamp_ms(),
            "msgs": [
                {
                    "cliMsgId": cli_msg_id,
                    "globalMsgId": msg_id,
                    "ownerId": uid_from,
                    "destId": thread_id,
                }
            ],
            "onlyMe": if only_me { 1 } else { 0 },
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/deletemsg"
        } else {
            "api/message/delete"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn forward_message(
        &self,
        message: &str,
        thread_ids: Vec<String>,
        thread_type: ThreadType,
        reference: Option<ForwardReference>,
        ttl: Option<u32>,
    ) -> Result<serde_json::Value, ZaloError> {
        if message.is_empty() {
            return Err(ZaloError::PayloadFormat(
                "Missing message content".to_string(),
            ));
        }
        if thread_ids.is_empty() {
            return Err(ZaloError::PayloadFormat("Missing thread IDs".to_string()));
        }

        let timestamp = get_timestamp_ms();
        let client_id = timestamp.to_string();

        let msg_info = serde_json::json!({
            "message": message,
            "reference": reference.as_ref().map(|r| {
                serde_json::json!({
                    "type": 3,
                    "data": serde_json::to_string(r).unwrap_or_default(),
                }).to_string()
            }),
        });

        let decor_log = reference.as_ref().map(|r| {
            serde_json::json!({
                "fw": {
                    "pmsg": {
                        "st": 1,
                        "ts": r.ts,
                        "id": r.id,
                    },
                    "rmsg": {
                        "st": 1,
                        "ts": r.ts,
                        "id": r.id,
                    },
                    "fwLvl": r.fw_lvl,
                }
            })
        });

        let is_group = thread_type == ThreadType::Group;
        let params = if !is_group {
            let to_ids: Vec<serde_json::Value> = thread_ids
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "clientId": client_id,
                        "toUid": id,
                        "ttl": ttl.unwrap_or(0),
                    })
                })
                .collect();
            serde_json::json!({
                "toIds": to_ids,
                "imei": self.ctx.imei,
                "ttl": ttl.unwrap_or(0),
                "msgType": "1",
                "totalIds": thread_ids.len(),
                "msgInfo": serde_json::to_string(&msg_info)?,
                "decorLog": decor_log.map(|d| serde_json::to_string(&d).unwrap_or_default()).unwrap_or_else(|| "null".to_string()),
            })
        } else {
            let grids: Vec<serde_json::Value> = thread_ids
                .iter()
                .map(|id| {
                    serde_json::json!({
                        "clientId": client_id,
                        "grid": id,
                        "ttl": ttl.unwrap_or(0),
                    })
                })
                .collect();
            serde_json::json!({
                "grids": grids,
                "ttl": ttl.unwrap_or(0),
                "msgType": "1",
                "totalIds": thread_ids.len(),
                "msgInfo": serde_json::to_string(&msg_info)?,
                "decorLog": decor_log.map(|d| serde_json::to_string(&d).unwrap_or_default()).unwrap_or_else(|| "null".to_string()),
            })
        };

        let service = "file";
        let path = if is_group {
            "api/group/mforward"
        } else {
            "api/message/mforward"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn send_bank_card(
        &self,
        bin_bank: crate::models::BinBankCard,
        num_acc_bank: &str,
        name_acc_bank: Option<&str>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let name_upper = name_acc_bank
            .map(|s| s.to_uppercase())
            .unwrap_or_else(|| "---".to_string());
        let params = serde_json::json!({
            "binBank": bin_bank,
            "numAccBank": num_acc_bank,
            "nameAccBank": name_upper,
            "cliMsgId": get_timestamp_ms().to_string(),
            "tsMsg": get_timestamp_ms(),
            "destUid": thread_id,
            "destType": if thread_type == ThreadType::Group { 1 } else { 0 },
        });

        self.request_api_form("zimsg", "api/transfer/card", params)
            .await
    }

    pub async fn send_card(
        &self,
        user_id: &str,
        phone_number: Option<&str>,
        ttl: Option<u32>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let qr_res = self.get_qr(user_id).await?;
        let qr_code_url = qr_res
            .get(user_id)
            .ok_or_else(|| ZaloError::ApiError {
                message: format!("Failed to get QR code for user {}", user_id),
                code: None,
            })?
            .clone();

        let client_id = get_timestamp_ms().to_string();

        let mut msg_info = serde_json::json!({
            "contactUid": user_id,
            "qrCodeUrl": qr_code_url,
        });

        if let Some(phone) = phone_number {
            msg_info
                .as_object_mut()
                .map(|m| m.insert("phone".to_string(), serde_json::json!(phone)));
        }

        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "ttl": ttl.unwrap_or(0),
            "msgType": 6,
            "clientId": client_id,
            "msgInfo": msg_info.to_string(),
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toId".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        }

        let service = "file";
        let path = if is_group {
            "api/group/forward"
        } else {
            "api/message/forward"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn send_link(
        &self,
        link: &str,
        msg: Option<&str>,
        ttl: Option<u32>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let res = self.parse_link(link).await?;

        let href = res
            .get("href")
            .and_then(|v| v.as_str())
            .unwrap_or(link)
            .to_string();
        let src = res
            .get("src")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let title = res
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let desc = res
            .get("desc")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let thumb = res
            .get("thumb")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let media = res.get("media").cloned().unwrap_or(serde_json::Value::Null);

        let final_msg = match msg {
            Some(m) if !m.trim().is_empty() => {
                if m.contains(link) {
                    m.to_string()
                } else {
                    format!("{} {}", m, link)
                }
            }
            _ => link.to_string(),
        };

        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "msg": final_msg,
            "href": href,
            "src": src,
            "title": title,
            "desc": desc,
            "thumb": thumb,
            "type": 2,
            "media": serde_json::to_string(&media).unwrap_or_else(|_| "{}".to_string()),
            "ttl": ttl.unwrap_or(0),
            "clientId": get_timestamp_ms(),
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toId".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("mentionInfo".to_string(), serde_json::json!("")));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/sendlink"
        } else {
            "api/message/link"
        };

        self.request_api_form_with_query(service, path, &[("nretry", "0")], params)
            .await
    }

    #[expect(clippy::too_many_arguments)]
    pub async fn send_video(
        &self,
        video_url: &str,
        thumbnail_url: &str,
        msg: Option<&str>,
        duration: Option<u32>,
        width: Option<u32>,
        height: Option<u32>,
        ttl: Option<u32>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let mut file_size: u64 = 0;
        let client_id = get_timestamp_ms();

        match self.ctx.reqwest_client.head(video_url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Some(len_str) = resp.headers().get("content-length") {
                        if let Ok(len_val) = len_str.to_str() {
                            file_size = len_val.parse::<u64>().unwrap_or(0);
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ZaloError::Crypto(format!(
                    "Unable to get video content: {}",
                    e
                )));
            }
        }

        let msg_info = serde_json::json!({
            "videoUrl": video_url,
            "thumbUrl": thumbnail_url,
            "duration": duration.unwrap_or(0),
            "width": width.unwrap_or(1280),
            "height": height.unwrap_or(720),
            "fileSize": file_size,
            "properties": {
                "color": -1,
                "size": -1,
                "type": 1003,
                "subType": 0,
                "ext": {
                    "sSrcType": -1,
                    "sSrcStr": "",
                    "msg_warning_type": 0,
                }
            },
            "title": msg.unwrap_or(""),
        });

        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "clientId": client_id.to_string(),
            "ttl": ttl.unwrap_or(0),
            "zsource": 704,
            "msgType": 5,
            "msgInfo": msg_info.to_string(),
            "imei": self.ctx.imei,
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toId".to_string(), serde_json::json!(thread_id)));
        }

        let service = "file";
        let path = if is_group {
            "api/group/forward"
        } else {
            "api/message/forward"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn send_voice(
        &self,
        voice_url: &str,
        ttl: Option<u32>,
        thread_id: &str,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        let mut file_size: u64 = 0;
        let client_id = get_timestamp_ms().to_string();

        match self.ctx.reqwest_client.head(voice_url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    if let Some(len_str) = resp.headers().get("content-length") {
                        if let Ok(len_val) = len_str.to_str() {
                            file_size = len_val.parse::<u64>().unwrap_or(0);
                        }
                    }
                }
            }
            Err(e) => {
                return Err(ZaloError::Crypto(format!(
                    "Unable to get voice content: {}",
                    e
                )));
            }
        }

        let msg_info = serde_json::json!({
            "voiceUrl": voice_url,
            "m4aUrl": voice_url,
            "fileSize": file_size,
        });

        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "clientId": client_id,
            "ttl": ttl.unwrap_or(0),
            "zsource": -1,
            "msgType": 3,
            "msgInfo": msg_info.to_string(),
            "imei": self.ctx.imei,
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
            params
                .as_object_mut()
                .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toId".to_string(), serde_json::json!(thread_id)));
        }

        let service = "file";
        let path = if is_group {
            "api/group/forward"
        } else {
            "api/message/forward"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn send_seen_event(
        &self,
        messages: Vec<SeenMessageParams>,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        if messages.is_empty() {
            return Err(ZaloError::PayloadFormat(
                "Messages are missing or empty".to_string(),
            ));
        }
        if messages.len() > 50 {
            return Err(ZaloError::PayloadFormat(
                "Cannot send seen event for more than 50 messages at once".to_string(),
            ));
        }

        let is_group = thread_type == ThreadType::Group;
        let first_msg = messages
            .first()
            .ok_or_else(|| ZaloError::PayloadFormat("Messages are missing or empty".to_string()))?;
        let thread_id = if is_group {
            &first_msg.id_to
        } else {
            &first_msg.uid_from
        };

        let my_uid = {
            let guard = self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            guard.clone().unwrap_or_default()
        };

        let mut data = Vec::new();
        for msg in &messages {
            let cur_thread_id = if is_group { &msg.id_to } else { &msg.uid_from };
            if cur_thread_id != thread_id {
                return Err(ZaloError::PayloadFormat(
                    "All messages must belong to the same thread".to_string(),
                ));
            }

            let ts_int = match &msg.ts {
                serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
                serde_json::Value::String(s) => s.parse::<i64>().unwrap_or(0),
                _ => 0,
            };

            data.push(serde_json::json!({
                "cmi": msg.cli_msg_id,
                "gmi": msg.msg_id,
                "si": msg.uid_from,
                "di": if msg.id_to == my_uid { "0" } else { &msg.id_to },
                "mt": msg.msg_type,
                "st": msg.st,
                "at": msg.at,
                "cmd": msg.cmd,
                "ts": ts_int,
            }));
        }

        let mut msg_infos = serde_json::json!({
            "data": data,
        });

        if is_group {
            msg_infos
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
        } else {
            msg_infos
                .as_object_mut()
                .map(|m| m.insert("senderId".to_string(), serde_json::json!(thread_id)));
        }

        let mut params = serde_json::json!({
            "msgInfos": msg_infos.to_string(),
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/seenv2"
        } else {
            "api/message/seenv2"
        };

        self.request_api_form_with_query(service, path, &[("nretry", "0")], params)
            .await
    }

    pub async fn send_delivered_event(
        &self,
        is_seen: bool,
        messages: Vec<SeenMessageParams>,
        thread_type: ThreadType,
    ) -> Result<serde_json::Value, ZaloError> {
        if messages.is_empty() {
            return Err(ZaloError::PayloadFormat(
                "Messages are missing or empty".to_string(),
            ));
        }
        if messages.len() > 50 {
            return Err(ZaloError::PayloadFormat(
                "Cannot send delivered event for more than 50 messages at once".to_string(),
            ));
        }

        let is_group = thread_type == ThreadType::Group;
        let first_msg = messages
            .first()
            .ok_or_else(|| ZaloError::PayloadFormat("Messages are missing or empty".to_string()))?;
        let id_to = &first_msg.id_to;
        if is_group && !messages.iter().all(|msg| &msg.id_to == id_to) {
            return Err(ZaloError::PayloadFormat(
                "All messages must have the same idTo for Group thread".to_string(),
            ));
        }

        let my_uid = {
            let guard = self.ctx.uid.lock().unwrap_or_else(|e| e.into_inner());
            guard.clone().unwrap_or_default()
        };

        let mut data = Vec::new();
        for msg in &messages {
            let ts_int = match &msg.ts {
                serde_json::Value::Number(n) => n.as_i64().unwrap_or(0),
                serde_json::Value::String(s) => s.parse::<i64>().unwrap_or(0),
                _ => 0,
            };

            data.push(serde_json::json!({
                "cmi": msg.cli_msg_id,
                "gmi": msg.msg_id,
                "si": msg.uid_from,
                "di": if msg.id_to == my_uid { "0" } else { &msg.id_to },
                "mt": msg.msg_type,
                "st": msg.st,
                "at": msg.at,
                "cmd": msg.cmd,
                "ts": ts_int,
            }));
        }

        let mut msg_infos = serde_json::json!({
            "seen": if is_seen { 1 } else { 0 },
            "data": data,
        });

        if is_group {
            msg_infos
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(id_to)));
        }

        let mut params = serde_json::json!({
            "msgInfos": msg_infos.to_string(),
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("imei".to_string(), serde_json::json!(self.ctx.imei)));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/deliveredv2"
        } else {
            "api/message/deliveredv2"
        };

        self.request_api_form(service, path, params).await
    }

    pub async fn send_typing_event(
        &self,
        thread_id: &str,
        thread_type: ThreadType,
        dest_type: Option<crate::models::DestType>,
    ) -> Result<serde_json::Value, ZaloError> {
        let is_group = thread_type == ThreadType::Group;
        let mut params = serde_json::json!({
            "imei": self.ctx.imei,
        });

        if is_group {
            params
                .as_object_mut()
                .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
        } else {
            params
                .as_object_mut()
                .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
            let dt = dest_type.unwrap_or(crate::models::DestType::User);
            params
                .as_object_mut()
                .map(|m| m.insert("destType".to_string(), serde_json::json!(dt)));
        }

        let service = if is_group { "group" } else { "chat" };
        let path = if is_group {
            "api/group/typing"
        } else {
            "api/message/typing"
        };

        self.request_api_form(service, path, params).await
    }
}

pub(crate) fn build_text_params(
    message: &str,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
    quote: Option<&SendMessageQuote>,
    mentions: Option<&[MessageMention]>,
    ttl: Option<u64>,
    imei: &str,
) -> serde_json::Value {
    let is_group = thread_type == ThreadType::Group;

    let mentions_val = if is_group {
        if let Some(m_list) = mentions {
            let filtered: Vec<serde_json::Value> = m_list
                .iter()
                .filter(|m| m.pos >= 0 && !m.uid.is_empty() && m.len > 0)
                .map(|m| {
                    serde_json::json!({
                        "pos": m.pos,
                        "uid": m.uid,
                        "len": m.len,
                        "type": if m.uid == "-1" { 1 } else { 0 },
                    })
                })
                .collect();
            if filtered.is_empty() {
                serde_json::Value::Null
            } else {
                serde_json::json!(filtered)
            }
        } else {
            serde_json::Value::Null
        }
    } else {
        serde_json::Value::Null
    };

    let has_mentions = !mentions_val.is_null();

    let mut params = serde_json::json!({
        "message": message,
        "clientId": client_id,
        "ttl": ttl.unwrap_or(0),
    });

    if is_group {
        params
            .as_object_mut()
            .map(|m| m.insert("grid".to_string(), serde_json::json!(thread_id)));
        params
            .as_object_mut()
            .map(|m| m.insert("visibility".to_string(), serde_json::json!(0)));
        if has_mentions {
            params.as_object_mut().map(|m| {
                m.insert(
                    "mentionInfo".to_string(),
                    serde_json::json!(mentions_val.to_string()),
                )
            });
        }
    } else {
        params
            .as_object_mut()
            .map(|m| m.insert("toid".to_string(), serde_json::json!(thread_id)));
        params
            .as_object_mut()
            .map(|m| m.insert("imei".to_string(), serde_json::json!(imei)));
    }

    if let Some(q) = quote {
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgOwner".to_string(), serde_json::json!(q.uid_from)));
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgId".to_string(), serde_json::json!(q.msg_id)));
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgCliId".to_string(), serde_json::json!(q.cli_msg_id)));

        let qmsg_type = match q.msg_type.as_str() {
            "webchat" => 1,
            _ => 1,
        };
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgType".to_string(), serde_json::json!(qmsg_type)));
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgTs".to_string(), serde_json::json!(q.ts)));
        params
            .as_object_mut()
            .map(|m| m.insert("qmsgTTL".to_string(), serde_json::json!(q.ttl)));

        let qmsg_content = if q.content.is_string() {
            q.content.as_str().unwrap_or("").to_string()
        } else {
            if q.msg_type == "chat.todo" {
                if let Some(params_str) = q.content.get("params").and_then(|v| v.as_str()) {
                    let parsed: Result<serde_json::Value, _> = serde_json::from_str(params_str);
                    if let Ok(p) = parsed {
                        p.get("item")
                            .and_then(|i| i.get("content"))
                            .and_then(|c| c.as_str())
                            .unwrap_or("")
                            .to_string()
                    } else {
                        "".to_string()
                    }
                } else {
                    "".to_string()
                }
            } else {
                "".to_string()
            }
        };
        params
            .as_object_mut()
            .map(|m| m.insert("qmsg".to_string(), serde_json::json!(qmsg_content)));

        if is_group {
            let qmsg_attach = if q.content.is_string() {
                q.property_ext.clone().unwrap_or(serde_json::Value::Null)
            } else if q.msg_type == "chat.todo" {
                serde_json::json!({
                    "properties": {
                        "color": 0,
                        "size": 0,
                        "type": 0,
                        "subType": 0,
                        "ext": "{\"shouldParseLinkOrContact\":0}"
                    }
                })
            } else {
                let mut attach_obj = q.content.clone();
                if let Some(thumb) = q.content.get("thumb") {
                    attach_obj
                        .as_object_mut()
                        .map(|m| m.insert("thumbUrl".to_string(), thumb.clone()));
                }
                if let Some(href) = q.content.get("href") {
                    attach_obj
                        .as_object_mut()
                        .map(|m| m.insert("oriUrl".to_string(), href.clone()));
                    attach_obj
                        .as_object_mut()
                        .map(|m| m.insert("normalUrl".to_string(), href.clone()));
                }
                attach_obj
            };
            params.as_object_mut().map(|m| {
                m.insert(
                    "qmsgAttach".to_string(),
                    serde_json::json!(qmsg_attach.to_string()),
                )
            });
        }
    }

    params
}

pub(crate) fn build_sticker_params(
    sticker_id: u32,
    cate_id: u32,
    sticker_type: u32,
    thread_id: &str,
    thread_type: ThreadType,
    client_id: u64,
    imei: &str,
) -> serde_json::Value {
    let is_group = thread_type == ThreadType::Group;
    let mut params = serde_json::json!({
        "stickerId": sticker_id,
        "cateId": cate_id,
        "type": sticker_type,
        "clientId": client_id,
        "imei": imei,
        "zsource": 101,
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

    params
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_text_client_id_is_preserved() {
        let params = build_text_params(
            "hello",
            "peer-1",
            ThreadType::User,
            1_785_491_200_123,
            None,
            None,
            None,
            "imei-a",
        );
        assert_eq!(params["clientId"], 1_785_491_200_123_u64);
        assert_eq!(params["toid"], "peer-1");
        assert_eq!(params["imei"], "imei-a");
    }

    #[test]
    fn explicit_sticker_client_id_is_preserved() {
        let params = build_sticker_params(
            100,
            20,
            1,
            "group-1",
            ThreadType::Group,
            1_785_491_200_456,
            "imei-a",
        );
        assert_eq!(params["clientId"], 1_785_491_200_456_u64);
        assert_eq!(params["grid"], "group-1");
        assert_eq!(params["stickerId"], 100);
        assert_eq!(params["cateId"], 20);
    }
}
