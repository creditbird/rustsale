import { Scroll, Text, View } from "nsh";
import { Progress } from "nsh/base";
import { Badge, Button, Modal } from "../ui";
import {
  closeUpdateModal,
  downloadAndInstallUpdate,
  formatBytes,
  relaunchToInstallUpdate,
  updaterState,
} from "../stores/updater";

/** Modal showing available software update details, download progress, install and relaunch (UI-45). */
export function UpdateModal() {
  return (
    <Modal
      testID="update-modal"
      open={updaterState().modalOpen}
      title="Cập nhật phần mềm mới"
      preventClose={updaterState().status === "downloading"}
      onClose={closeUpdateModal}
      class="m-auto w-[520px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
    >
      <View class="gap-4">
        <View class="flex-row items-center justify-between gap-2">
          <Text class="text-xs text-muted-foreground">
            RustSale CRM & Outreach Pro
          </Text>
          <If when={updaterState().availableVersion !== null}>
            <Badge
              label={`v${updaterState().availableVersion ?? ""}`}
              tone="success"
            />
          </If>
        </View>

        <View class="flex-row gap-3 rounded-(--radius) border border-border bg-muted/40 p-3">
          <View class="flex-1 gap-1">
            <Text class="text-xs text-muted-foreground">Phiên bản hiện tại</Text>
            <Text testID="update-current-version" class="text-sm font-semibold text-foreground">
              {`v${updaterState().currentVersion}`}
            </Text>
          </View>
          <View class="flex-1 gap-1 border-l border-border pl-3">
            <Text class="text-xs text-muted-foreground">Phiên bản mới nhất</Text>
            <Text testID="update-available-version" class="text-sm font-semibold text-success">
              {`v${updaterState().availableVersion ?? "---"}`}
            </Text>
          </View>
        </View>

        <View class="gap-1.5">
          <Text class="text-xs font-semibold uppercase text-foreground">
            Ghi chú phát hành
          </Text>
          <View class="rounded-(--radius) border border-border bg-background">
            <Scroll class="h-32 p-3">
              <If when={(updaterState().releaseNotes ?? "").trim() !== ""}>
                <Text testID="update-release-notes" class="text-xs text-foreground">
                  {updaterState().releaseNotes ?? ""}
                </Text>
              </If>
              <If when={(updaterState().releaseNotes ?? "").trim() === ""}>
                <Text testID="update-release-notes" class="text-xs text-muted-foreground">
                  Không có ghi chú phát hành chi tiết.
                </Text>
              </If>
            </Scroll>
          </View>
        </View>

        <If when={updaterState().status === "downloading"}>
          <View testID="update-progress-box" class="gap-2 rounded-(--radius) border border-primary/30 bg-primary/5 p-3">
            <View class="flex-row items-center justify-between">
              <Text class="text-xs font-medium text-foreground">
                Đang tải bản cập nhật…
              </Text>
              <Text testID="update-progress-pct" class="text-xs font-semibold text-primary">
                {`${updaterState().progress.percentage}%`}
              </Text>
            </View>
            <Progress
              value={updaterState().progress.percentage}
              indeterminate={updaterState().progress.total <= 0}
              accessibilityLabel="Tiến độ tải bản cập nhật"
              trackClass="h-2 rounded-full bg-muted"
              indicatorClass="h-2 rounded-full bg-primary"
            />
            <View class="flex-row items-center justify-between">
              <Text testID="update-progress-bytes" class="text-xs text-muted-foreground">
                {`Đã tải: ${formatBytes(updaterState().progress.received)}`}
              </Text>
              <If when={updaterState().progress.total > 0}>
                <Text class="text-xs text-muted-foreground">
                  {`Tổng: ${formatBytes(updaterState().progress.total)}`}
                </Text>
              </If>
            </View>
          </View>
        </If>

        <If when={updaterState().status === "downloaded"}>
          <View testID="update-downloaded-banner" class="gap-1 rounded-(--radius) border border-success/30 bg-success/10 p-3">
            <Text class="text-xs font-semibold text-success">
              Tải bản cập nhật thành công!
            </Text>
            <Text class="text-xs text-muted-foreground">
              Khởi động lại ứng dụng để hoàn tất cài đặt phiên bản mới.
            </Text>
          </View>
        </If>

        <If when={updaterState().status === "error"}>
          <View testID="update-error-banner" class="gap-1 rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3">
            <Text class="text-xs font-semibold text-destructive">
              Cập nhật thất bại
            </Text>
            <Text class="text-xs text-destructive">
              {updaterState().errorMessage ?? "Vui lòng kiểm tra kết nối mạng và thử lại."}
            </Text>
          </View>
        </If>

        <If when={updaterState().status === "available" && !updaterState().canInstall}>
          <View testID="update-manual-banner" class="gap-1 rounded-(--radius) border border-warning/30 bg-warning/10 p-3">
            <Text class="text-xs font-semibold text-warning">
              Cần cập nhật thủ công
            </Text>
            <Text class="text-xs text-muted-foreground">
              {updaterState().releaseUrl
                ? `Tải bản mới tại: ${updaterState().releaseUrl}`
                : "Bản cài đặt hiện tại không hỗ trợ tự ghi đè."}
            </Text>
          </View>
        </If>

        <View class="flex-row items-center justify-end gap-2 border-t border-border pt-3">
          <If when={updaterState().status === "downloaded"}>
            <Button
              testID="update-later"
              size="sm"
              label="Để sau"
              onPress={closeUpdateModal}
            />
            <Button
              testID="update-restart"
              size="sm"
              variant="primary"
              label="Khởi động lại ngay"
              onPress={() => { void relaunchToInstallUpdate(); }}
            />
          </If>

          <If when={updaterState().status === "downloading"}>
            <Button
              testID="update-downloading"
              size="sm"
              label="Đang cập nhật…"
              disabled
              onPress={() => {}}
            />
          </If>

          <If when={updaterState().status !== "downloaded" && updaterState().status !== "downloading"}>
            <Button
              testID="update-later"
              size="sm"
              variant="ghost"
              label="Để sau"
              onPress={closeUpdateModal}
            />
            <Button
              testID="update-now"
              size="sm"
              variant="primary"
              label="Cập nhật ngay"
              disabled={updaterState().availableVersion === null || !updaterState().canInstall}
              onPress={() => { void downloadAndInstallUpdate(); }}
            />
          </If>
        </View>
      </View>
    </Modal>
  );
}
