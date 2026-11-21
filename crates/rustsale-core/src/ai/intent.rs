use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CustomerIntent {
    AskingPrice,
    ProductInquiry,
    ScheduleMeeting,
    Complaint,
    Unsubscribe,
    GeneralInquiry,
}

impl CustomerIntent {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::AskingPrice => "asking_price",
            Self::ProductInquiry => "product_inquiry",
            Self::ScheduleMeeting => "schedule_meeting",
            Self::Complaint => "complaint",
            Self::Unsubscribe => "unsubscribe",
            Self::GeneralInquiry => "general_inquiry",
        }
    }

    pub fn display_name_vi(&self) -> &'static str {
        match self {
            Self::AskingPrice => "Hỏi giá / Báo giá",
            Self::ProductInquiry => "Tư vấn sản phẩm / Tính năng",
            Self::ScheduleMeeting => "Hẹn gặp / Xem hàng",
            Self::Complaint => "Khiếu nại / Phản ánh",
            Self::Unsubscribe => "Hủy nhận tin / Từ chối",
            Self::GeneralInquiry => "Chào hỏi / Khác",
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "asking_price" | "askingprice" | "price" | "hỏi giá" | "báo giá" => {
                Some(Self::AskingPrice)
            }
            "product_inquiry" | "productinquiry" | "product" | "tư vấn sản phẩm" => {
                Some(Self::ProductInquiry)
            }
            "schedule_meeting" | "schedule" | "hẹn gặp" | "xem hàng" => {
                Some(Self::ScheduleMeeting)
            }
            "complaint" | "khiếu nại" | "phản ánh" => Some(Self::Complaint),
            "unsubscribe" | "hủy nhận tin" | "từ chối" => Some(Self::Unsubscribe),
            "general_inquiry" | "general" | "chào hỏi" => Some(Self::GeneralInquiry),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentResult {
    pub intent: CustomerIntent,
    pub confidence: f64,
    pub detected_language: String,
    pub matched_keywords: Vec<String>,
    pub reasoning: String,
    pub is_heuristic_match: bool,
}

pub struct IntentClassifier;

impl IntentClassifier {
    pub fn classify_heuristics(text: &str) -> IntentResult {
        let cleaned = text.trim();
        if cleaned.is_empty() {
            return IntentResult {
                intent: CustomerIntent::GeneralInquiry,
                confidence: 0.50,
                detected_language: "vi".to_string(),
                matched_keywords: vec![],
                reasoning: "Empty query defaulted to GeneralInquiry".to_string(),
                is_heuristic_match: true,
            };
        }

        let lower = cleaned.to_lowercase();
        let unaccented = remove_vietnamese_accents(&lower);

        // Define intent rule sets with weights
        let unsubscribe_rules = [
            ("đừng gửi", "dung gui", 0.98),
            ("ngừng gửi", "ngung gui", 0.98),
            ("đừng nhắn", "dung nhan", 0.98),
            ("đừng làm phiền", "dung lam phien", 0.99),
            ("không muốn nhận", "khong muon nhan", 0.95),
            ("hủy nhận tin", "huy nhan tin", 0.99),
            ("xóa số", "xoa so", 0.95),
            ("chặn số", "chan so", 0.95),
            ("bỏ theo dõi", "bo theo doi", 0.95),
            ("phiền quá", "phien qua", 0.90),
            ("stop", "stop", 0.95),
            ("unsubscribe", "unsubscribe", 0.99),
            ("remove me", "remove me", 0.95),
            ("don't message", "don't message", 0.95),
            ("spam", "spam", 0.85),
        ];

        let complaint_rules = [
            ("khiếu nại", "khieu nai", 0.98),
            ("hàng lỗi", "hang loi", 0.98),
            ("lỗi rồi", "loi roi", 0.98),
            ("bị lỗi", "bi loi", 0.95),
            ("hỏng rồi", "hong roi", 0.98),
            ("bị hỏng", "bi hong", 0.95),
            ("thất vọng", "that vong", 0.95),
            ("quá tệ", "qua te", 0.95),
            ("chất lượng kém", "chat luong kem", 0.95),
            ("hoàn tiền", "hoan tien", 0.98),
            ("trả hàng", "tra hang", 0.95),
            ("đổi trả", "doi tra", 0.90),
            ("lừa đảo", "lua dao", 0.99),
            ("bực mình", "buc minh", 0.90),
            ("giao sai", "giao sai", 0.95),
            ("chưa nhận được hàng", "chua nhan duoc hang", 0.92),
            ("complaint", "complaint", 0.95),
            ("broken", "broken", 0.95),
            ("defective", "defective", 0.95),
            ("refund", "refund", 0.95),
            ("scam", "scam", 0.95),
        ];

        let schedule_rules = [
            ("mai qua", "mai qua", 0.95),
            ("chiều nay qua", "chieu nay qua", 0.95),
            ("sáng mai qua", "sang mai qua", 0.95),
            ("qua xem hàng", "qua xem hang", 0.98),
            ("đến xem hàng", "den xem hang", 0.98),
            ("tới xem", "toi xem", 0.95),
            ("hẹn gặp", "hen gap", 0.98),
            ("đặt lịch", "dat lich", 0.95),
            ("lên lịch", "len lich", 0.95),
            ("mấy giờ mở cửa", "may gio mo cua", 0.90),
            ("địa chỉ ở đâu", "dia chi o dau", 0.85),
            ("qua shop", "qua shop", 0.90),
            ("xem demo", "xem demo", 0.92),
            ("gọi tư vấn cho mình", "goi tu van cho minh", 0.88),
            ("appointment", "appointment", 0.95),
            ("schedule", "schedule", 0.95),
            ("visit store", "visit store", 0.95),
            ("demo", "demo", 0.85),
        ];

        let price_rules = [
            ("giá bao nhiêu", "gia bao nhieu", 0.99),
            ("báo giá", "bao gia", 0.98),
            ("xin giá", "xin gia", 0.95),
            ("bnhieu", "bnhieu", 0.95),
            ("bn tiền", "bn tien", 0.95),
            ("bao tiền", "bao tien", 0.95),
            ("inbox giá", "inbox gia", 0.98),
            ("ib giá", "ib gia", 0.98),
            ("chi phí thế nào", "chi phi the nao", 0.95),
            ("bảng giá", "bang gia", 0.95),
            ("bao nhiêu một", "bao nhieu mot", 0.95),
            ("giá sao", "gia sao", 0.95),
            ("giá thế nào", "gia the nao", 0.95),
            ("có giảm giá không", "co giam gia khong", 0.90),
            ("chiết khấu", "chiet khau", 0.90),
            ("có fix không", "co fix khong", 0.90),
            ("giá bn", "gia bn", 0.95),
            ("phí ship", "phi ship", 0.85),
            ("how much", "how much", 0.98),
            ("price", "price", 0.90),
            ("cost", "cost", 0.90),
            ("quote", "quote", 0.90),
        ];

        let product_rules = [
            ("tư vấn sản phẩm", "tu van san pham", 0.95),
            ("tư vấn giúp", "tu van giup", 0.90),
            ("tư vấn cho mình", "tu van cho minh", 0.90),
            ("thông tin sản phẩm", "thong tin san pham", 0.95),
            ("tính năng", "tinh nang", 0.90),
            ("bên bạn có", "ben ban co", 0.88),
            ("shop có bán", "shop co ban", 0.88),
            ("còn hàng không", "con hang khong", 0.92),
            ("hàng có sẵn", "hang co san", 0.90),
            ("mẫu mã", "mau ma", 0.88),
            ("chất liệu", "chat lieu", 0.88),
            ("kích thước", "kich thuoc", 0.88),
            ("màu sắc", "mau sac", 0.88),
            ("có những loại nào", "co nhung loai nao", 0.90),
            ("xem mẫu", "xem mau", 0.90),
            ("product info", "product info", 0.90),
            ("specifications", "specifications", 0.90),
            ("available", "available", 0.85),
        ];

        let general_rules = [
            ("chào shop", "chao shop", 0.85),
            ("alo", "alo", 0.80),
            ("hello", "hello", 0.80),
            ("hi shop", "hi shop", 0.80),
            ("xin chào", "xin chao", 0.85),
            ("có ai không", "co ai khong", 0.80),
        ];

        // 1. Check Unsubscribe first (High priority safety gate)
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &unsubscribe_rules) {
            return IntentResult {
                intent: CustomerIntent::Unsubscribe,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched unsubscribe / opt-out keyword pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // 2. Check Complaint (High priority CRM risk gate)
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &complaint_rules) {
            return IntentResult {
                intent: CustomerIntent::Complaint,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched complaint / issue keyword pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // 3. Check ScheduleMeeting
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &schedule_rules) {
            return IntentResult {
                intent: CustomerIntent::ScheduleMeeting,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched meeting / appointment scheduling pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // 4. Check AskingPrice
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &price_rules) {
            return IntentResult {
                intent: CustomerIntent::AskingPrice,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched pricing inquiry keyword pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // 5. Check ProductInquiry
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &product_rules) {
            return IntentResult {
                intent: CustomerIntent::ProductInquiry,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched product information inquiry pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // 6. Check GeneralInquiry
        if let Some((kw, conf)) = match_rules(&lower, &unaccented, &general_rules) {
            return IntentResult {
                intent: CustomerIntent::GeneralInquiry,
                confidence: conf,
                detected_language: detect_lang(&lower),
                matched_keywords: vec![kw],
                reasoning: "Matched general greeting pattern".to_string(),
                is_heuristic_match: true,
            };
        }

        // Fallback heuristic check: single words like "giá", "tư vấn", "lỗi"
        if lower.contains("giá") || unaccented.contains("gia") || lower.contains("price") {
            return IntentResult {
                intent: CustomerIntent::AskingPrice,
                confidence: 0.85,
                detected_language: detect_lang(&lower),
                matched_keywords: vec!["giá".to_string()],
                reasoning: "Fallback match on price word".to_string(),
                is_heuristic_match: true,
            };
        }

        if lower.contains("tư vấn") || unaccented.contains("tu van") {
            return IntentResult {
                intent: CustomerIntent::ProductInquiry,
                confidence: 0.80,
                detected_language: detect_lang(&lower),
                matched_keywords: vec!["tư vấn".to_string()],
                reasoning: "Fallback match on consultation word".to_string(),
                is_heuristic_match: true,
            };
        }

        IntentResult {
            intent: CustomerIntent::GeneralInquiry,
            confidence: 0.60,
            detected_language: detect_lang(&lower),
            matched_keywords: vec![],
            reasoning: "Unclassified text defaulted to GeneralInquiry".to_string(),
            is_heuristic_match: true,
        }
    }

    pub async fn classify(
        text: &str,
        gateway: Option<&crate::ai::gateway::LlmGateway>,
    ) -> IntentResult {
        let heuristic = Self::classify_heuristics(text);
        let gateway = match gateway {
            Some(gateway) if heuristic.confidence < 0.80 => gateway,
            _ => return heuristic,
        };
        let system_prompt = "You are an AI CRM Intent Classifier for Vietnamese customer messages.\nCategorize into exactly one of: asking_price, product_inquiry, schedule_meeting, complaint, unsubscribe, general_inquiry.\nReturn ONLY valid JSON: {\"intent\": \"...\", \"confidence\": 0.95, \"reasoning\": \"...\"}";
        let messages = [crate::ai::gateway::LlmMessage::user(format!(
            "Classify message: \"{}\"",
            text
        ))];

        match gateway
            .chat_completion(&messages, Some(system_prompt))
            .await
        {
            Ok(resp) => {
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(&resp.content) {
                    if let Some(intent_str) = json["intent"].as_str() {
                        if let Some(parsed_intent) = CustomerIntent::from_str(intent_str) {
                            let conf = json["confidence"].as_f64().unwrap_or(0.85);
                            let reason = json["reasoning"]
                                .as_str()
                                .unwrap_or("LLM classification")
                                .to_string();
                            return IntentResult {
                                intent: parsed_intent,
                                confidence: conf,
                                detected_language: detect_lang(text),
                                matched_keywords: vec![],
                                reasoning: reason,
                                is_heuristic_match: false,
                            };
                        }
                    }
                }
                heuristic
            }
            Err(_) => heuristic,
        }
    }
}

fn match_rules(
    lower: &str,
    unaccented: &str,
    rules: &[(&str, &str, f64)],
) -> Option<(String, f64)> {
    for (accented_rule, unaccented_rule, conf) in rules {
        if lower.contains(accented_rule) || unaccented.contains(unaccented_rule) {
            return Some((accented_rule.to_string(), *conf));
        }
    }
    None
}

fn detect_lang(text: &str) -> String {
    let lower = text.to_lowercase();
    let vi_chars = [
        'à', 'á', 'ả', 'ã', 'ạ', 'ă', 'ằ', 'ắ', 'ẳ', 'ẵ', 'ặ', 'â', 'ầ', 'ấ', 'ẩ', 'ẫ', 'ậ', 'đ',
        'è', 'é', 'ẻ', 'ẽ', 'ẹ', 'ê', 'ề', 'ế', 'ể', 'ễ', 'ệ', 'ì', 'í', 'ỉ', 'ĩ', 'ị', 'ò', 'ó',
        'ỏ', 'õ', 'ọ', 'ô', 'ồ', 'ố', 'ổ', 'ỗ', 'ộ', 'ơ', 'ờ', 'ớ', 'ở', 'ỡ', 'ợ', 'ù', 'ú', 'ủ',
        'ũ', 'ụ', 'ư', 'ừ', 'ứ', 'ử', 'ữ', 'ự', 'ỳ', 'ý', 'ỷ', 'ỹ', 'ỵ',
    ];
    if lower.chars().any(|c| vi_chars.contains(&c)) {
        "vi".to_string()
    } else {
        "en".to_string()
    }
}

pub fn remove_vietnamese_accents(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        let replacement = match c {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ'
            | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => {
                'e'
            }
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ'
            | 'ở' | 'ỡ' | 'ợ' => 'o',
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'đ' => 'd',
            _ => c,
        };
        result.push(replacement);
    }
    result
}
