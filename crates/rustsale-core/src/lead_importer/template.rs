use std::io::{Cursor, Write};
use zip::write::SimpleFileOptions;
use zip::ZipWriter;

pub const TEMPLATE_HEADERS: &[&str] = &[
    "Họ và tên (*)",
    "Số điện thoại",
    "Email",
    "Công ty",
    "Giá trị (VND)",
    "Trạng thái",
    "Địa chỉ",
    "Ghi chú",
];

pub const SAMPLE_ROW_1: &[&str] = &[
    "Nguyễn Văn An",
    "0912345678",
    "vanan@example.com",
    "Bất Động Sản Á Châu",
    "50000000",
    "Mới",
    "123 Cầu Giấy, Hà Nội",
    "Quan tâm căn hộ 2 phòng ngủ",
];

pub const SAMPLE_ROW_2: &[&str] = &[
    "Trần Thị Bình",
    "0987654321",
    "binh.tran@tech.vn",
    "FPT Software",
    "0",
    "Đang chăm sóc",
    "Quận 1, TP. Hồ Chí Minh",
    "Khách liên hệ từ sự kiện",
];

pub fn generate_csv_lead_template() -> Vec<u8> {
    let mut out = Vec::new();
    // UTF-8 BOM so Excel opens with correct Vietnamese accents
    out.extend_from_slice(b"\xEF\xBB\xBF");

    let header_line = TEMPLATE_HEADERS
        .iter()
        .map(|h| format!("\"{}\"", h))
        .collect::<Vec<_>>()
        .join(",");
    out.extend_from_slice(header_line.as_bytes());
    out.extend_from_slice(b"\r\n");

    let row1 = SAMPLE_ROW_1
        .iter()
        .map(|val| format!("\"{}\"", val.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",");
    out.extend_from_slice(row1.as_bytes());
    out.extend_from_slice(b"\r\n");

    let row2 = SAMPLE_ROW_2
        .iter()
        .map(|val| format!("\"{}\"", val.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(",");
    out.extend_from_slice(row2.as_bytes());
    out.extend_from_slice(b"\r\n");

    out
}

pub fn generate_excel_lead_template() -> Result<Vec<u8>, String> {
    let buffer = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(buffer);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o644);

    // 1. [Content_Types].xml
    zip.start_file("[Content_Types].xml", options)
        .map_err(|e| format!("Lỗi tạo [Content_Types].xml: {}", e))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">
  <Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>
  <Default Extension="xml" ContentType="application/xml"/>
  <Override PartName="/xl/workbook.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet.main+xml"/>
  <Override PartName="/xl/worksheets/sheet1.xml" ContentType="application/vnd.openxmlformats-officedocument.spreadsheetml.worksheet+xml"/>
</Types>"#,
    )
    .map_err(|e| format!("Lỗi ghi [Content_Types].xml: {}", e))?;

    // 2. _rels/.rels
    zip.start_file("_rels/.rels", options)
        .map_err(|e| format!("Lỗi tạo _rels/.rels: {}", e))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="xl/workbook.xml"/>
</Relationships>"#,
    )
    .map_err(|e| format!("Lỗi ghi _rels/.rels: {}", e))?;

    // 3. xl/_rels/workbook.xml.rels
    zip.start_file("xl/_rels/workbook.xml.rels", options)
        .map_err(|e| format!("Lỗi tạo workbook.xml.rels: {}", e))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">
  <Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/worksheet" Target="worksheets/sheet1.xml"/>
</Relationships>"#,
    )
    .map_err(|e| format!("Lỗi ghi workbook.xml.rels: {}", e))?;

    // 4. xl/workbook.xml
    zip.start_file("xl/workbook.xml", options)
        .map_err(|e| format!("Lỗi tạo workbook.xml: {}", e))?;
    zip.write_all(
        br#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<workbook xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships">
  <sheets>
    <sheet name="Leads" sheetId="1" r:id="rId1"/>
  </sheets>
</workbook>"#,
    )
    .map_err(|e| format!("Lỗi ghi workbook.xml: {}", e))?;

    // 5. xl/worksheets/sheet1.xml
    zip.start_file("xl/worksheets/sheet1.xml", options)
        .map_err(|e| format!("Lỗi tạo sheet1.xml: {}", e))?;

    let mut sheet_content = String::from(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<worksheet xmlns="http://schemas.openxmlformats.org/spreadsheetml/2006/main">
  <sheetData>
"#,
    );

    // Row 1: Headers
    sheet_content.push_str("    <row r=\"1\">\n");
    let col_letters = ["A", "B", "C", "D", "E", "F", "G", "H"];
    for (i, h) in TEMPLATE_HEADERS.iter().enumerate() {
        let col = col_letters.get(i).unwrap_or(&"A");
        sheet_content.push_str(&format!(
            "      <c r=\"{}1\" t=\"inlineStr\"><is><t>{}</t></is></c>\n",
            col,
            xml_escape(h)
        ));
    }
    sheet_content.push_str("    </row>\n");

    // Row 2: Sample 1
    sheet_content.push_str("    <row r=\"2\">\n");
    for (i, val) in SAMPLE_ROW_1.iter().enumerate() {
        let col = col_letters.get(i).unwrap_or(&"A");
        sheet_content.push_str(&format!(
            "      <c r=\"{}2\" t=\"inlineStr\"><is><t>{}</t></is></c>\n",
            col,
            xml_escape(val)
        ));
    }
    sheet_content.push_str("    </row>\n");

    // Row 3: Sample 2
    sheet_content.push_str("    <row r=\"3\">\n");
    for (i, val) in SAMPLE_ROW_2.iter().enumerate() {
        let col = col_letters.get(i).unwrap_or(&"A");
        sheet_content.push_str(&format!(
            "      <c r=\"{}3\" t=\"inlineStr\"><is><t>{}</t></is></c>\n",
            col,
            xml_escape(val)
        ));
    }
    sheet_content.push_str("    </row>\n");
    sheet_content.push_str("  </sheetData>\n</worksheet>");

    zip.write_all(sheet_content.as_bytes())
        .map_err(|e| format!("Lỗi ghi sheet1.xml: {}", e))?;

    let cursor = zip
        .finish()
        .map_err(|e| format!("Lỗi hoàn tất đóng gói zip xlsx: {}", e))?;

    Ok(cursor.into_inner())
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
