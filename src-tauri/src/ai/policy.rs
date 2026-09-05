use crate::ai::evidence::LeadEvidenceContext;
use crate::ai::intent::CustomerIntent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyDecision {
    ApproveAuto,
    RequireReview { reasons: Vec<String> },
    Reject { reasons: Vec<String> },
}

impl PolicyDecision {
    pub fn is_approved(&self) -> bool {
        matches!(self, Self::ApproveAuto)
    }

    pub fn decision_name(&self) -> &'static str {
        match self {
            Self::ApproveAuto => "APPROVE_AUTO",
            Self::RequireReview { .. } => "REQUIRE_REVIEW",
            Self::Reject { .. } => "REJECT",
        }
    }

    pub fn reasons(&self) -> Vec<String> {
        match self {
            Self::ApproveAuto => vec![],
            Self::RequireReview { reasons } => reasons.clone(),
            Self::Reject { reasons } => reasons.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyConfig {
    pub min_confidence_threshold: f64,
    pub forbidden_words: Vec<String>,
    pub require_review_for_complaints: bool,
    pub require_review_for_price_quotes: bool,
    pub require_review_for_deals_above: Option<f64>,
    pub max_auto_approved_discount_percent: f64,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            min_confidence_threshold: 0.75,
            forbidden_words: vec![
                // Offensive / Profanity
                "đm".to_string(),
                "dm".to_string(),
                "dmm".to_string(),
                "vcl".to_string(),
                "vl".to_string(),
                "đĩ".to_string(),
                "chó chết".to_string(),
                "khốn nạn".to_string(),
                "đồ lừa đảo".to_string(),
                "fuck".to_string(),
                "bitch".to_string(),
                "scam".to_string(),
                // Phishing / Malicious links
                "bit.ly/scam".to_string(),
                "chuyển tiền ngay vào tài khoản cá nhân".to_string(),
            ],
            require_review_for_complaints: true,
            require_review_for_price_quotes: true,
            require_review_for_deals_above: Some(50_000_000.0), // 50 million VND
            max_auto_approved_discount_percent: 0.0,
        }
    }
}

pub struct PolicyGate {
    config: PolicyConfig,
}

impl PolicyGate {
    pub fn new(config: PolicyConfig) -> Self {
        Self { config }
    }

    pub fn with_default_config() -> Self {
        Self::new(PolicyConfig::default())
    }

    pub fn config(&self) -> &PolicyConfig {
        &self.config
    }

    pub fn evaluate_proposed_reply(
        &self,
        proposed_text: &str,
        intent: &CustomerIntent,
        confidence: f64,
        evidence: &LeadEvidenceContext,
    ) -> PolicyDecision {
        let text_lower = proposed_text.to_lowercase();
        let unaccented_text = crate::ai::intent::remove_vietnamese_accents(&text_lower);
        let mut reject_reasons = Vec::new();
        let mut review_reasons = Vec::new();

        // 1. Check Forbidden / Offensive Words -> Instant REJECT
        for word in &self.config.forbidden_words {
            let w_lower = word.to_lowercase();
            let w_unacc = crate::ai::intent::remove_vietnamese_accents(&w_lower);
            if text_lower.contains(&w_lower) || unaccented_text.contains(&w_unacc) {
                reject_reasons.push(format!("Phát hiện từ cấm / phản cảm: \"{}\"", word));
            }
        }

        // 2. Unsubscribe Intent Safety Check
        if *intent == CustomerIntent::Unsubscribe {
            // If customer wants to unsubscribe, the reply MUST acknowledge opt-out and MUST NOT push sales
            let is_apologetic = text_lower.contains("ngừng")
                || text_lower.contains("hủy")
                || text_lower.contains("xin lỗi")
                || text_lower.contains("không làm phiền")
                || text_lower.contains("thông cảm")
                || text_lower.contains("đã xóa");

            let is_pushy = text_lower.contains("khuyến mãi")
                || text_lower.contains("mua ngay")
                || text_lower.contains("giảm giá")
                || text_lower.contains("đặt hàng");

            if is_pushy {
                reject_reasons.push("Khách hàng yêu cầu hủy nhận tin, không được gửi thông điệp bán hàng/khuyến mãi".to_string());
            } else if !is_apologetic {
                review_reasons.push("Cần nhân viên xác nhận việc hủy đăng ký nhận tin của khách hàng".to_string());
            }
        }

        if !reject_reasons.is_empty() {
            return PolicyDecision::Reject {
                reasons: reject_reasons,
            };
        }

        // 3. Confidence Threshold Check
        if confidence < self.config.min_confidence_threshold {
            review_reasons.push(format!(
                "Độ tin cậy AI ({:.0}%) dưới ngưỡng an toàn ({:.0}%)",
                confidence * 100.0,
                self.config.min_confidence_threshold * 100.0
            ));
        }

        // 4. Complaint Intent Check -> Require Human Review
        if *intent == CustomerIntent::Complaint && self.config.require_review_for_complaints {
            review_reasons.push("Khách hàng đang khiếu nại/phản ánh sự cố, bắt buộc chuyên viên duyệt phản hồi".to_string());
        }

        // 5. High Value Deal Threshold
        if let Some(threshold) = self.config.require_review_for_deals_above {
            let lead_val = evidence.deal_value.unwrap_or(evidence.lead_value);
            if lead_val >= threshold {
                review_reasons.push(format!(
                    "Hợp đồng giá trị cao ({:.0} {} >= {:.0} {}), cần phê duyệt",
                    lead_val, evidence.lead_currency, threshold, evidence.lead_currency
                ));
            }
        }

        // 6. Pricing / Discount Commitment Rule
        if *intent == CustomerIntent::AskingPrice {
            let mentions_price = text_lower.contains("giá")
                || text_lower.contains("đ")
                || text_lower.contains("k")
                || text_lower.contains("vnd")
                || text_lower.contains("triệu");

            if mentions_price && evidence.known_products.is_empty() && evidence.lead_value == 0.0 {
                review_reasons.push("Báo giá cho khách hàng nhưng chưa có dữ liệu giá niêm yết trong CRM để đối chiếu".to_string());
            } else if self.config.require_review_for_price_quotes {
                review_reasons.push("Cam kết về giá hoặc báo giá chính thức cần nhân viên kinh doanh xác nhận".to_string());
            }
        }

        // 7. Check if reply is excessively short or empty
        if proposed_text.trim().len() < 5 {
            review_reasons.push("Nội dung phản hồi quá ngắn hoặc thiếu thông tin".to_string());
        }

        if !review_reasons.is_empty() {
            PolicyDecision::RequireReview {
                reasons: review_reasons,
            }
        } else {
            PolicyDecision::ApproveAuto
        }
    }
}
