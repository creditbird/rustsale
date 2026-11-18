use crate::ai::evidence::LeadEvidenceContext;
use crate::ai::gateway::{LlmGateway, LlmMessage};
use crate::ai::intent::{CustomerIntent, IntentResult};
use crate::ai::policy::PolicyGate;
use chrono::SecondsFormat;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartReplyVariant {
    pub variant_type: String,
    pub title: String,
    pub content: String,
    pub suggested_action: Option<String>,
    pub confidence: f64,
    pub policy_decision: String,
    pub policy_reasons: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartReplyOutput {
    pub lead_id: i32,
    pub conversation_id: Option<i64>,
    pub intent: IntentResult,
    pub evidence_summary: String,
    pub variants: Vec<SmartReplyVariant>,
    pub generated_at: String,
}

pub struct SmartReplyGenerator;

impl SmartReplyGenerator {
    pub fn build_prompt(
        context: &LeadEvidenceContext,
        intent: &IntentResult,
        custom_prompt: Option<&str>,
    ) -> String {
        let evidence_text = context.to_context_summary();
        let custom_instruction = custom_prompt
            .map(|p| format!("\n[Yêu cầu bổ sung từ nhân viên kinh doanh]: {}\n", p))
            .unwrap_or_default();

        format!(
            r#"Bạn là Trợ lý AI Bán Hàng & Chăm Sóc Khách Hàng chuyên nghiệp của RustSale CRM.
Nhiệm vụ: Phân tích thông tin khách hàng và soạn thảo 3 phương án phản hồi (variants) bằng Tiếng Việt chuẩn mực, lịch sự, đúng sự thật và không bịa đặt thông tin.

[Dữ liệu xác thực từ CRM (Evidence Ledger)]:
{}

[Ý định khách hàng đã nhận diện]:
- Intent: {} ({})
- Độ tin cậy: {:.0}%
{}
Hãy tạo đúng 3 phương án phản hồi dạng JSON:
1. "brief" (Ngắn gọn / Thân thiện): 1-2 câu súc tích cho chat nhanh.
2. "detailed" (Chi tiết / Đầy đủ): Đầy đủ thông tin, giải đáp rõ thắc mắc, bám sát dữ liệu CRM.
3. "promotional" (Kêu gọi hành động / Khuyến mãi): Thúc đẩy chốt deal, hẹn lịch tư vấn hoặc ưu đãi.

Yêu cầu định dạng: Trả về DUY NHẤT một chuỗi JSON hợp lệ không kèm markdown giải thích:
{{
  "variants": [
    {{
      "variant_type": "brief",
      "title": "Ngắn gọn / Thân thiện",
      "content": "...",
      "suggested_action": "Gửi tin nhắn nhanh",
      "confidence": 0.95
    }},
    {{
      "variant_type": "detailed",
      "title": "Chi tiết / Đầy đủ",
      "content": "...",
      "suggested_action": "Gửi thông tin chi tiết",
      "confidence": 0.90
    }},
    {{
      "variant_type": "promotional",
      "title": "Kêu gọi hành động",
      "content": "...",
      "suggested_action": "Mời hẹn gặp / Đặt lịch",
      "confidence": 0.88
    }}
  ]
}}"#,
            evidence_text,
            intent.intent.as_str(),
            intent.intent.display_name_vi(),
            intent.confidence * 100.0,
            custom_instruction
        )
    }

    pub async fn generate(
        context: &LeadEvidenceContext,
        intent: &IntentResult,
        custom_prompt: Option<&str>,
        gateway: &LlmGateway,
        policy_gate: &PolicyGate,
    ) -> Result<SmartReplyOutput, String> {
        let now = chrono::Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true);
        let evidence_summary = context.to_context_summary();

        let prompt = Self::build_prompt(context, intent, custom_prompt);
        let messages = [LlmMessage::user(prompt)];
        let system_prompt =
            "You are an expert Vietnamese sales copilot in RustSale CRM. Output valid JSON only.";

        let llm_resp = gateway
            .chat_completion(&messages, Some(system_prompt))
            .await;

        let mut variants = Vec::new();

        if let Ok(resp) = llm_resp {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.content) {
                if let Some(arr) = json["variants"].as_array() {
                    for v in arr {
                        let v_type = v["variant_type"].as_str().unwrap_or("general").to_string();
                        let title = v["title"].as_str().unwrap_or("Gợi ý phản hồi").to_string();
                        let content = v["content"].as_str().unwrap_or("").to_string();
                        let action = v["suggested_action"].as_str().map(|s| s.to_string());
                        let conf = v["confidence"].as_f64().unwrap_or(0.85);

                        if !content.trim().is_empty() {
                            let decision = policy_gate.evaluate_proposed_reply(
                                &content,
                                &intent.intent,
                                conf,
                                context,
                            );

                            variants.push(SmartReplyVariant {
                                variant_type: v_type,
                                title,
                                content,
                                suggested_action: action,
                                confidence: conf,
                                policy_decision: decision.decision_name().to_string(),
                                policy_reasons: decision.reasons(),
                            });
                        }
                    }
                }
            }
        }

        // Fallback to offline deterministic templates if LLM didn't produce 3 variants
        if variants.len() < 3 {
            variants = Self::generate_offline_variants(context, intent, policy_gate);
        }

        Ok(SmartReplyOutput {
            lead_id: context.lead_id,
            conversation_id: None,
            intent: intent.clone(),
            evidence_summary,
            variants,
            generated_at: now,
        })
    }

    pub fn generate_offline_variants(
        context: &LeadEvidenceContext,
        intent: &IntentResult,
        policy_gate: &PolicyGate,
    ) -> Vec<SmartReplyVariant> {
        let name = &context.lead_name;
        let company = if context.lead_company.trim().is_empty() {
            "quý công ty".to_string()
        } else {
            context.lead_company.clone()
        };

        let product_info = if let Some(prod) = context.known_products.first() {
            format!(
                "gói \"{}\" với mức giá {:.0} {}",
                prod.name, prod.price, prod.currency
            )
        } else if context.lead_value > 0.0 {
            format!(
                "sản phẩm phù hợp với ngân sách {:.0} {}",
                context.lead_value, context.lead_currency
            )
        } else {
            "các giải pháp phần mềm & dịch vụ chuyên nghiệp bên em".to_string()
        };

        let raw_variants = match intent.intent {
            CustomerIntent::AskingPrice => vec![
                (
                    "brief",
                    "Ngắn gọn / Báo giá nhanh",
                    format!("Dạ em chào anh/chị {}, hiện tại {} đang áp dụng mức giá ưu đãi nhất ạ. Em xin phép gửi bảng giá chi tiết qua tin nhắn nhé ạ!", name, product_info),
                    Some("Gửi bảng giá nhanh".to_string()),
                    0.95,
                ),
                (
                    "detailed",
                    "Chi tiết / Kèm chính sách",
                    format!("Dạ em chào anh/chị {}, bên em xin gửi thông tin chi tiết về {}. Mức giá đã bao gồm toàn bộ chi phí triển khai, đào tạo và bảo hành hỗ trợ kỹ thuật 24/7 cho {} ạ. Anh/chị có cần em gửi báo giá PDF qua email ({}) không ạ?", name, product_info, company, context.lead_email),
                    Some("Gửi hồ sơ báo giá đầy đủ".to_string()),
                    0.92,
                ),
                (
                    "promotional",
                    "Kêu gọi chốt deal / Khuyến mãi",
                    format!("Dạ chào anh/chị {}, đặc biệt trong tháng này khi đăng ký {}, bên em đang có chính sách chiết khấu thêm và tặng kèm gói hỗ trợ nâng cao. Anh/chị cho em xin lịch trao đổi 10 phút để nhận ưu đãi này nhé!", name, product_info),
                    Some("Hẹn cuộc gọi tư vấn ưu đãi".to_string()),
                    0.88,
                ),
            ],
            CustomerIntent::ScheduleMeeting => vec![
                (
                    "brief",
                    "Xác nhận lịch hẹn nhanh",
                    format!("Dạ em chào anh/chị {}, bên em rất sẵn sàng đón tiếp mình qua xem và trải nghiệm sản phẩm trực tiếp ạ!", name),
                    Some("Xác nhận lịch hẹn".to_string()),
                    0.95,
                ),
                (
                    "detailed",
                    "Đầy đủ địa chỉ & Khung giờ",
                    format!("Dạ chào anh/chị {}, văn phòng bên em mở cửa từ 8h30 - 18h00 các ngày trong tuần. Anh/chị tiện qua buổi sáng (9h-11h) hay buổi chiều (14h-17h) để chuyên viên bên em chuẩn bị đón tiếp chu đáo nhất cho {} ạ?", name, company),
                    Some("Lựa chọn khung giờ hẹn".to_string()),
                    0.93,
                ),
                (
                    "promotional",
                    "Mời Demo 1-1 Cá nhân hóa",
                    format!("Dạ bên em có thể sắp xếp buổi Demo 1-1 trực tiếp hoặc online qua Google Meet để trình diễn cụ thể giải pháp cho {}. Anh/chị chọn ngày giờ thuận tiện nhất, em sẽ gửi link mời ngay nhé ạ!", company),
                    Some("Gửi link đặt lịch online".to_string()),
                    0.90,
                ),
            ],
            CustomerIntent::Complaint => vec![
                (
                    "brief",
                    "Tiếp nhận & Xin lỗi nhanh",
                    format!("Dạ em chào anh/chị {}, em rất xin lỗi vì sự bất tiện này ạ. Em sẽ kiểm tra ngay thông tin sự cố và phản hồi lại anh/chị trong ít phút tới ạ!", name),
                    Some("Tiếp nhận sự cố khẩn cấp".to_string()),
                    0.90,
                ),
                (
                    "detailed",
                    "Quy trình xử lý & Hỗ trợ kỹ thuật",
                    format!("Dạ em chào anh/chị {}, thay mặt đội ngũ, em thành thật xin lỗi vì trải nghiệm chưa trọn vẹn của {} vừa qua. Em đã chuyển ngay vấn đề tới bộ phận Kỹ thuật/CSKH cấp cao để xử lý ưu tiên số 1 cho mình. Bên em sẽ liên hệ lại qua số điện thoại {} ngay lập tức ạ.", name, company, context.lead_phone),
                    Some("Chuyển phòng CSKH can thiệp".to_string()),
                    0.88,
                ),
                (
                    "promotional",
                    "Cam kết quyền lợi & Bồi hoàn",
                    "Dạ bên em cam kết bảo hành và đổi mới 100% nếu có bất kỳ sai sót nào từ hệ thống. Để bù đắp sự bất tiện, em xin gửi tặng anh/chị phiếu quà tặng và ưu tiên hỗ trợ VIP trực tiếp cho mình ạ.".to_string(),
                    Some("Áp dụng chính sách đền bù/hỗ trợ".to_string()),
                    0.85,
                ),
            ],
            CustomerIntent::Unsubscribe => vec![
                (
                    "brief",
                    "Xác nhận hủy nhận tin",
                    format!("Dạ vâng em chào anh/chị {}, em đã ghi nhận và ngừng gửi tin nhắn làm phiền mình rồi ạ. Chúc anh/chị một ngày tốt lành!", name),
                    Some("Cập nhật trạng thái ngừng liên hệ".to_string()),
                    0.98,
                ),
                (
                    "detailed",
                    "Xác nhận xóa khỏi danh sách",
                    format!("Dạ em chào anh/chị {}, em vô cùng xin lỗi nếu tin nhắn vừa qua làm phiền mình. Em đã xóa số điện thoại {} khỏi danh sách thông báo của bên em. Khi nào có nhu cầu, rất mong được đón tiếp anh/chị trong tương lai ạ.", name, context.lead_phone),
                    Some("Hủy đăng ký & Lưu vết CRM".to_string()),
                    0.95,
                ),
                (
                    "promotional",
                    "Chào tạm biệt lịch sự",
                    format!("Dạ vâng ạ, em cảm ơn anh/chị {} đã dành thời gian phản hồi. Chúc {} luôn phát triển thuận lợi và thành công ạ!", name, company),
                    Some("Đóng hội thoại lịch sự".to_string()),
                    0.92,
                ),
            ],
            CustomerIntent::ProductInquiry => vec![
                (
                    "brief",
                    "Tư vấn tổng quan",
                    format!("Dạ em chào anh/chị {}, bên em hiện có đầy đủ các giải pháp cho {} theo đúng nhu cầu của mình ạ!", name, company),
                    Some("Gửi tài liệu giới thiệu".to_string()),
                    0.95,
                ),
                (
                    "detailed",
                    "Chi tiết tính năng & Lợi ích",
                    format!("Dạ chào anh/chị {}, về sản phẩm/dịch vụ này, hệ thống hỗ trợ tự động hóa toàn diện, đồng bộ dữ liệu đa kênh và báo cáo quản trị chuyên sâu cho {}. Em xin phép gửi catalog chi tiết kèm video hướng dẫn qua cho anh/chị tham khảo nhé ạ!", name, company),
                    Some("Gửi Catalog & Video Demo".to_string()),
                    0.92,
                ),
                (
                    "promotional",
                    "Trải nghiệm dùng thử miễn phí",
                    format!("Dạ bên em đang mở chương trình trải nghiệm giải pháp miễn phí 14 ngày dành riêng cho doanh nghiệp trong ngành của {}. Em kích hoạt tài khoản thử nghiệm cho anh/chị ngay nhé ạ?", company),
                    Some("Kích hoạt tài khoản dùng thử".to_string()),
                    0.90,
                ),
            ],
            CustomerIntent::GeneralInquiry => vec![
                (
                    "brief",
                    "Chào hỏi thân thiện",
                    format!("Dạ em chào anh/chị {}, em có thể hỗ trợ gì cho mình hôm nay ạ?", name),
                    Some("Phản hồi chào đón".to_string()),
                    0.95,
                ),
                (
                    "detailed",
                    "Giới thiệu hỗ trợ toàn diện",
                    format!("Dạ em chào anh/chị {}, em là trợ lý tư vấn từ RustSale. Em thấy {} đang quan tâm đến giải pháp của bên em, em có thể hỗ trợ giải đáp mọi thắc mắc hoặc báo giá cụ thể cho mình ạ!", name, company),
                    Some("Mở đầu tư vấn chuyên sâu".to_string()),
                    0.92,
                ),
                (
                    "promotional",
                    "Kết nối chuyên viên tư vấn",
                    format!("Dạ chào anh/chị {}, nếu anh/chị cần khảo sát giải pháp tối ưu chi phí cho {}, em xin phép kết nối chuyên viên tư vấn trực tiếp cho mình trong 5 phút nữa nhé ạ!", name, company),
                    Some("Kết nối chuyên viên bán hàng".to_string()),
                    0.88,
                ),
            ],
        };

        raw_variants
            .into_iter()
            .map(|(v_type, title, content, action, conf)| {
                let decision =
                    policy_gate.evaluate_proposed_reply(&content, &intent.intent, conf, context);

                SmartReplyVariant {
                    variant_type: v_type.to_string(),
                    title: title.to_string(),
                    content,
                    suggested_action: action,
                    confidence: conf,
                    policy_decision: decision.decision_name().to_string(),
                    policy_reasons: decision.reasons(),
                }
            })
            .collect()
    }
}
