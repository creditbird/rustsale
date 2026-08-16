#!/usr/bin/env bash
set -e

# ==============================================================================
# RustSale CRM - Universal Linux One-Line Installer
# ==============================================================================
# Usage:
#   curl -fsSL https://rustsale.trantrongtri.com/install-linux.sh | bash
# ==============================================================================

BOLD="\033[1m"
GREEN="\033[0;32m"
CYAN="\033[0;36m"
YELLOW="\033[0;33m"
RED="\033[0;31m"
RESET="\033[0m"

echo -e "${CYAN}${BOLD}"
echo "  ____            _   ____        _        "
echo " |  _ \ _   _ ___| |_/ ___|  __ _| | ___   "
echo " | |_) | | | / __| __\___ \ / _\` | |/ _ \  "
echo " |  _ <| |_| \__ \ |_ ___) | (_| | |  __/  "
echo " |_| \_\\__,_|___/\__|____/ \__,_|_|\___|  "
echo "                                           "
echo "  CRM & Multi-channel Outreach Platform    "
echo -e "${RESET}"

# 1. Check architecture
ARCH=$(uname -m)
if [ "$ARCH" != "x86_64" ]; then
  echo -e "${RED}❌ Kiến trúc $ARCH chưa được hỗ trợ. Trình cài đặt yêu cầu x86_64 (amd64).${RESET}"
  exit 1
fi

# 2. Check required tools
for tool in curl tar; do
  if ! command -v $tool >/dev/null 2>&1; then
    echo -e "${RED}❌ Lỗi: Cần cài đặt '$tool' để tiếp tục.${RESET}"
    exit 1
  fi
done

BASE_URL="https://rustsale.trantrongtri.com"
TMP_DIR=$(mktemp -d)
trap 'rm -rf "$TMP_DIR"' EXIT

echo -e "${CYAN}📡 Đang kiểm tra phiên bản mới nhất từ Cloudflare R2...${RESET}"
MANIFEST_URL="${BASE_URL}/latest.json"

VERSION=$(curl -sSL "$MANIFEST_URL" | grep -o '"version": *"[^"]*"' | head -1 | cut -d'"' -f4)
if [ -z "$VERSION" ]; then
  VERSION="0.1.1"
fi

echo -e "${GREEN}✅ Tìm thấy phiên bản:${RESET} ${BOLD}v${VERSION}${RESET}"

# 3. Download package
DEB_URL="${BASE_URL}/v${VERSION}/RustSale%20CRM_${VERSION}_amd64.deb"
DEB_FILE="${TMP_DIR}/rustsale.deb"

echo -e "${CYAN}📥 Đang tải RustSale CRM v${VERSION} (~50MB)...${RESET}"
if ! curl -# -fSL "$DEB_URL" -o "$DEB_FILE"; then
  # Fallback to dot naming if needed
  DEB_URL="${BASE_URL}/v${VERSION}/RustSale.CRM_${VERSION}_amd64.deb"
  curl -# -fSL "$DEB_URL" -o "$DEB_FILE" || {
    echo -e "${RED}❌ Không thể tải bộ cài đặt từ $DEB_URL${RESET}"
    exit 1
  }
fi

# 4. Extract package
echo -e "${CYAN}📦 Đang giải nén và cài đặt ứng dụng...${RESET}"
cd "$TMP_DIR"

if command -v ar >/dev/null 2>&1; then
  ar x "$DEB_FILE"
elif command -v 7z >/dev/null 2>&1; then
  7z x "$DEB_FILE" >/dev/null
  7z x data.tar >/dev/null
elif command -v dpkg-deb >/dev/null 2>&1; then
  dpkg-deb -x "$DEB_FILE" "$TMP_DIR/extracted"
fi

# Find data.tar.*
DATA_TAR=$(ls data.tar.* 2>/dev/null | head -1 || true)
INSTALL_PREFIX="$HOME/.local"

copy_extracted_files() {
  local src_root="$1"
  mkdir -p "$INSTALL_PREFIX/bin" "$INSTALL_PREFIX/share/applications" "$INSTALL_PREFIX/share/icons"
  if [ -d "$src_root/usr/bin" ]; then
    for bin in "$src_root/usr/bin/"*; do
      if [ -f "$bin" ]; then
        install -m 755 "$bin" "$INSTALL_PREFIX/bin/$(basename "$bin")"
      fi
    done
  fi
  if [ -d "$src_root/usr/share/icons" ]; then
    cp -r -f --remove-destination "$src_root/usr/share/icons/"* "$INSTALL_PREFIX/share/icons/" 2>/dev/null || true
  fi
  if [ -d "$src_root/usr/share/applications" ]; then
    cp -r -f --remove-destination "$src_root/usr/share/applications/"* "$INSTALL_PREFIX/share/applications/" 2>/dev/null || true
  fi
}

if [ -n "$DATA_TAR" ]; then
  tar -xf "$DATA_TAR" -C "$TMP_DIR"
  copy_extracted_files "$TMP_DIR"
elif [ -d "$TMP_DIR/extracted" ]; then
  copy_extracted_files "$TMP_DIR/extracted"
else
  echo -e "${RED}❌ Lỗi: Không thể trích xuất file cài đặt.${RESET}"
  exit 1
fi

# 5. Create symlink & adjust permissions
chmod +x "$INSTALL_PREFIX/bin/rustsale-crm" "$INSTALL_PREFIX/bin/cloak-runtime" 2>/dev/null || true
ln -sf "$INSTALL_PREFIX/bin/rustsale-crm" "$INSTALL_PREFIX/bin/rustsale"

# 6. Fix .desktop entry
DESKTOP_FILE=$(find "$INSTALL_PREFIX/share/applications" -name "*ust*ale*.desktop" | head -1 || true)
if [ -n "$DESKTOP_FILE" ]; then
  sed -i "s|^Exec=.*|Exec=$INSTALL_PREFIX/bin/rustsale-crm %U|g" "$DESKTOP_FILE"
  sed -i "s|^Icon=.*|Icon=rustsale-crm|g" "$DESKTOP_FILE"
  chmod +x "$DESKTOP_FILE"
fi

# Update desktop database
if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$INSTALL_PREFIX/share/applications" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
  gtk-update-icon-cache -f -t "$INSTALL_PREFIX/share/icons/hicolor" 2>/dev/null || true
fi

# 7. Check PATH
if [[ ":$PATH:" != *":$HOME/.local/bin:"* ]]; then
  SHELL_RC=""
  if [ -n "$ZSH_VERSION" ] || [ "$SHELL" = "/bin/zsh" ]; then
    SHELL_RC="$HOME/.zshrc"
  elif [ -n "$BASH_VERSION" ] || [ "$SHELL" = "/bin/bash" ]; then
    SHELL_RC="$HOME/.bashrc"
  fi
  if [ -n "$SHELL_RC" ] && [ -f "$SHELL_RC" ]; then
    if ! grep -q 'export PATH="$HOME/.local/bin:$PATH"' "$SHELL_RC"; then
      echo 'export PATH="$HOME/.local/bin:$PATH"' >> "$SHELL_RC"
    fi
  fi
fi

echo ""
echo -e "${GREEN}${BOLD}======================================================${RESET}"
echo -e "${GREEN}${BOLD}🎉 CÀI ĐẶT RUSTSALE CRM v${VERSION} THÀNH CÔNG!${RESET}"
echo -e "${GREEN}${BOLD}======================================================${RESET}"
echo -e "📂 Thư mục cài đặt:  ${CYAN}$INSTALL_PREFIX/bin/rustsale-crm${RESET}"
echo -e "🚀 Cách khởi chạy:"
echo -e "   1. Nhấp vào icon ${BOLD}RustSale CRM${RESET} trong Menu ứng dụng (Application Launcher)."
echo -e "   2. Hoặc mở Terminal gõ lệnh: ${CYAN}${BOLD}rustsale${RESET}"
echo -e "${GREEN}${BOLD}======================================================${RESET}\n"
