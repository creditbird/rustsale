import { Image, Scroll, signal, Text, View } from "nsh";
import { Button as Base } from "nsh/base";
import { Badge, Button, Input, Modal } from "../ui";
import {
  closeLicenseModal,
  copyLicenseHwid,
  copyPaymentMemo,
  createLicensePayment,
  formatLicenseDate,
  formatVnd,
  licenseConfigured,
  licenseCopiedHwid,
  licenseCopiedMemo,
  licenseDevices,
  licenseError,
  licenseHwInfo,
  licenseLoading,
  licenseModalOpen,
  licensePlans,
  licensePolling,
  licensePreventClose,
  licenseSelectedPlan,
  licenseSuccess,
  licenseTab,
  licenseTransaction,
  licenseUser,
  licenseVerification,
  logoutLicense,
  refreshLicenseVerification,
  registerLicense,
  resetLicensePayment,
  revokeLicenseDevice,
  selectLicensePlan,
  signInLicense,
  transferMemoFor,
  type LicenseTab,
} from "../stores/license";

const TABS: { id: LicenseTab; label: string }[] = [
  { id: "status", label: "Trạng thái" },
  { id: "devices", label: "Thiết bị" },
  { id: "upgrade", label: "Nâng cấp / Thanh toán" },
  { id: "auth", label: "Tài khoản" },
];

function invalidReasonTitle(): string {
  const v = licenseVerification();
  if (!v) return "Bản quyền chưa được xác thực";
  if (v.reason === "subscription_expired") return "Gói bản quyền đã hết hạn";
  if (v.reason === "device_limit_exceeded") {
    return `Đã vượt giới hạn thiết bị (${v.current_devices ?? 0}/${v.max_devices ?? 0} thiết bị)`;
  }
  if (v.reason === "device_blocked") return "Thiết bị này đã bị khoá";
  if (v.reason === "unauthenticated") return "Chưa đăng nhập tài khoản bản quyền";
  if (v.reason === "unconfigured") return "Chưa cấu hình máy chủ bản quyền";
  return "Bản quyền chưa được xác thực";
}

function selectedPlanLabel(): string {
  const plan = licensePlans().find((p) => p.id === licenseSelectedPlan());
  if (!plan) return "Tạo đơn thanh toán";
  return `Tạo đơn thanh toán (${plan.name} - ${formatVnd(plan.price)})`;
}

function activeTransactionPlanName(): string {
  const tx = licenseTransaction();
  if (!tx) return "";
  const plan = licensePlans().find((p) => p.id === tx.plan_id);
  return plan?.name ?? tx.plan_id.toUpperCase();
}

/** Modal for License & HWID management, subscription upgrade, payment polling and startup gate (UI-46). */
export function LicenseModal() {
  const authEmail = signal("");
  const authPassword = signal("");
  const isRegisterMode = signal(false);
  const confirmRevokeId = signal<string | null>(null);

  const submitAuth = async () => {
    if (licenseLoading()) return;
    if (isRegisterMode()) {
      const ok = await registerLicense(authEmail(), authPassword());
      if (ok) {
        isRegisterMode.set(false);
        authPassword.set("");
      }
    } else {
      await signInLicense(authEmail(), authPassword());
      authPassword.set("");
    }
  };

  const confirmRevoke = async (deviceId: string) => {
    confirmRevokeId.set(null);
    await revokeLicenseDevice(deviceId);
  };

  return (
    <Modal
      testID="license-modal"
      open={licenseModalOpen()}
      title="Bản quyền & Quản lý HWID"
      preventClose={licensePreventClose()}
      onClose={closeLicenseModal}
      class="m-auto w-[640px] max-w-full rounded-(--radius) border border-border bg-popover p-5"
    >
      <View class="gap-3">
        <View class="flex-row items-center justify-between gap-2">
          <Text class="text-xs text-muted-foreground">
            Quản lý Hardware ID, thiết bị và gói bản quyền RustSale
          </Text>
          <If when={licensePreventClose()}>
            <View testID="license-gate-badge">
              <Badge label="Cần kích hoạt" tone="warning" />
            </View>
          </If>
          <If when={!licensePreventClose()}>
            <Button
              testID="license-close"
              size="sm"
              variant="ghost"
              label="Đóng"
              onPress={closeLicenseModal}
            />
          </If>
        </View>

        <View class="flex-row flex-wrap gap-2 border-b border-border pb-2">
          <Each of={TABS} key={(t) => t.id}>
            {(t) => (
              <Base
                testID={`license-tab-${t().id}`}
                accessibilityLabel={t().label}
                onPress={() => licenseTab.set(t().id)}
                class={`rounded-full border px-3 py-1 ${
                  licenseTab() === t().id
                    ? "border-primary bg-primary"
                    : "border-border bg-card hover:bg-accent"
                }`}
              >
                <Text
                  class={`text-xs font-medium ${
                    licenseTab() === t().id ? "text-primary-foreground" : "text-foreground"
                  }`}
                >
                  {t().id === "devices"
                    ? `${t().label} (${licenseDevices().length})`
                    : t().label}
                </Text>
              </Base>
            )}
          </Each>
        </View>

        <Scroll class="max-h-[560px]">
          <View class="gap-3 pr-1">
            <If when={licensePreventClose()}>
              <View
                testID="license-gate-banner"
                class="gap-1 rounded-(--radius) border border-warning/30 bg-warning/10 p-3"
              >
                <Text class="text-xs font-semibold text-warning">
                  Yêu cầu đăng nhập & kích hoạt bản quyền
                </Text>
                <Text class="text-xs text-muted-foreground">
                  Vui lòng đăng nhập hoặc gia hạn gói thiết bị để tiếp tục sử dụng RustSale.
                </Text>
              </View>
            </If>

            <If when={!licenseConfigured()}>
              <View
                testID="license-unconfigured-banner"
                class="gap-1 rounded-(--radius) border border-border bg-muted/40 p-3"
              >
                <Text class="text-xs font-medium text-foreground">
                  Chưa cấu hình máy chủ bản quyền trong bản dựng này.
                </Text>
                <Text class="text-xs text-muted-foreground">
                  Bạn vẫn có thể xem và sao chép HWID của thiết bị này bên dưới.
                </Text>
              </View>
            </If>

            <If when={licenseError() !== null}>
              <View
                testID="license-error"
                class="rounded-(--radius) border border-destructive/30 bg-destructive/10 p-3"
              >
                <Text class="text-xs text-destructive">{licenseError() ?? ""}</Text>
              </View>
            </If>

            <If when={licenseSuccess() !== null}>
              <View
                testID="license-success"
                class="rounded-(--radius) border border-success/30 bg-success/10 p-3"
              >
                <Text class="text-xs text-success">{licenseSuccess() ?? ""}</Text>
              </View>
            </If>

            {/* TAB: STATUS */}
            <If when={licenseTab() === "status"}>
              <View class="gap-3">
                <View class="gap-2 rounded-(--radius) border border-border bg-muted/40 p-3">
                  <View class="flex-row items-center justify-between gap-2">
                    <Text class="text-xs font-medium text-muted-foreground">
                      Hardware ID (HWID)
                    </Text>
                    <Button
                      testID="license-copy-hwid"
                      size="sm"
                      variant="secondary"
                      label={licenseCopiedHwid() ? "Đã sao chép" : "Sao chép HWID"}
                      disabled={licenseHwInfo() === null}
                      onPress={() => { void copyLicenseHwid(); }}
                    />
                  </View>
                  <View class="rounded-(--radius) border border-border bg-background px-3 py-2">
                    <Text testID="license-hwid-value" class="font-mono text-sm font-semibold text-success">
                      {licenseHwInfo()?.hardware_id ?? "Đang tải HWID…"}
                    </Text>
                  </View>
                  <View class="flex-row flex-wrap gap-4">
                    <Text class="text-xs text-muted-foreground">
                      {`Tên máy: ${licenseHwInfo()?.hostname ?? "—"}`}
                    </Text>
                    <Text class="text-xs text-muted-foreground">
                      {`Hệ điều hành: ${licenseHwInfo()?.os_info ?? "—"}`}
                    </Text>
                  </View>
                </View>

                <If when={licenseVerification() !== null && licenseVerification()?.allowed === true}>
                  <View
                    testID="license-status-valid"
                    class="gap-2 rounded-(--radius) border border-success/30 bg-success/10 p-3"
                  >
                    <Text class="text-sm font-semibold text-success">
                      Bản quyền hợp lệ — Đã cấp quyền truy cập
                    </Text>
                    <View class="gap-1 border-t border-success/20 pt-2">
                      <Text class="text-xs text-foreground">
                        {`Gói hiện tại: ${(licenseVerification()?.plan_id ?? "—").toUpperCase()}`}
                      </Text>
                      <Text class="text-xs text-foreground">
                        {`Hiệu lực đến: ${formatLicenseDate(licenseVerification()?.expires_at)}`}
                      </Text>
                      <Text class="text-xs text-foreground">
                        {`Số thiết bị tối đa: ${licenseVerification()?.max_devices ?? 0} thiết bị`}
                      </Text>
                    </View>
                  </View>
                </If>

                <If when={licenseVerification() !== null && licenseVerification()?.allowed === false && licenseConfigured()}>
                  <View
                    testID="license-status-invalid"
                    class="gap-1 rounded-(--radius) border border-warning/30 bg-warning/10 p-3"
                  >
                    <Text class="text-sm font-semibold text-warning">
                      {invalidReasonTitle()}
                    </Text>
                    <Text class="text-xs text-muted-foreground">
                      Vui lòng đăng nhập, gia hạn gói bản quyền hoặc thu hồi thiết bị cũ trong tab Thiết bị.
                    </Text>
                  </View>
                </If>

                <View class="flex-row justify-end">
                  <Button
                    testID="license-recheck"
                    size="sm"
                    variant="primary"
                    label={licenseLoading() ? "Đang kiểm tra…" : "Kiểm tra lại"}
                    disabled={licenseLoading()}
                    onPress={() => { void refreshLicenseVerification(); }}
                  />
                </View>
              </View>
            </If>

            {/* TAB: DEVICES */}
            <If when={licenseTab() === "devices"}>
              <View class="gap-2.5">
                <Text class="text-xs text-muted-foreground">
                  Danh sách thiết bị đang liên kết với tài khoản này:
                </Text>
                <If when={licenseDevices().length === 0}>
                  <View testID="license-devices-empty" class="rounded-(--radius) border border-border bg-muted/20 p-4">
                    <Text class="text-center text-xs text-muted-foreground">
                      Chưa có thiết bị nào được đăng ký.
                    </Text>
                  </View>
                </If>
                <Each of={licenseDevices()} key={(d) => d.id}>
                  {(dev) => (
                    <View
                      testID={`license-device-${dev().id}`}
                      class="gap-2 rounded-(--radius) border border-border bg-card p-3"
                    >
                      <View class="flex-row items-center justify-between gap-2">
                        <View class="flex-1 gap-0.5">
                          <View class="flex-row items-center gap-2">
                            <Text class="text-xs font-semibold text-foreground">
                              {dev().device_name}
                            </Text>
                            <If when={licenseHwInfo()?.hardware_id === dev().hardware_id}>
                              <Badge label="Thiết bị hiện tại" tone="success" />
                            </If>
                          </View>
                          <Text class="font-mono text-xs text-muted-foreground">
                            {dev().hardware_id}
                          </Text>
                          <Text class="text-xs text-muted-foreground">
                            {`Hoạt động gần nhất: ${formatLicenseDate(dev().last_active)}`}
                          </Text>
                        </View>

                        <If when={confirmRevokeId() !== dev().id}>
                          <Button
                            testID={`license-revoke-${dev().id}`}
                            size="sm"
                            variant="destructive"
                            label="Thu hồi"
                            disabled={licenseLoading()}
                            onPress={() => confirmRevokeId.set(dev().id)}
                          />
                        </If>
                        <If when={confirmRevokeId() === dev().id}>
                          <View class="flex-row gap-1.5">
                            <Button
                              testID={`license-cancel-revoke-${dev().id}`}
                              size="sm"
                              variant="ghost"
                              label="Huỷ"
                              onPress={() => confirmRevokeId.set(null)}
                            />
                            <Button
                              testID={`license-confirm-revoke-${dev().id}`}
                              size="sm"
                              variant="destructive"
                              label="Xác nhận thu hồi"
                              disabled={licenseLoading()}
                              onPress={() => { void confirmRevoke(dev().id); }}
                            />
                          </View>
                        </If>
                      </View>
                    </View>
                  )}
                </Each>
              </View>
            </If>

            {/* TAB: UPGRADE / PAYMENT */}
            <If when={licenseTab() === "upgrade"}>
              <View class="gap-3">
                <View class="flex-row gap-2">
                  <Each of={licensePlans()} key={(p) => p.id}>
                    {(plan) => (
                      <Base
                        testID={`license-plan-${plan().id}`}
                        accessibilityLabel={plan().name}
                        onPress={() => selectLicensePlan(plan().id)}
                        class={`flex-1 gap-1 rounded-(--radius) border p-3 ${
                          licenseSelectedPlan() === plan().id
                            ? "border-primary bg-primary/5"
                            : "border-border bg-card hover:bg-accent"
                        }`}
                      >
                        <Text class="text-sm font-semibold text-foreground">
                          {plan().name}
                        </Text>
                        <Text class="text-sm font-semibold text-success">
                          {`${formatVnd(plan().price)} / ${plan().duration_days} ngày`}
                        </Text>
                        <Text class="text-xs text-muted-foreground">
                          {plan().description}
                        </Text>
                      </Base>
                    )}
                  </Each>
                </View>

                <If when={licenseTransaction() === null}>
                  <Button
                    testID="license-create-payment"
                    variant="primary"
                    label={selectedPlanLabel()}
                    disabled={licenseLoading() || !licenseConfigured()}
                    onPress={() => { void createLicensePayment(); }}
                  />
                </If>

                <If when={licenseTransaction() !== null}>
                  <View
                    testID="license-payment-panel"
                    class="items-center gap-2.5 rounded-(--radius) border border-border bg-muted/30 p-3"
                  >
                    <Text class="text-xs font-semibold text-foreground">
                      {`Quét mã QR để thanh toán (${activeTransactionPlanName()})`}
                    </Text>

                    <If when={Boolean(licenseTransaction()?.qr_code_url)}>
                      <Image
                        testID="license-payment-qr"
                        src={licenseTransaction()?.qr_code_url ?? ""}
                        class="h-36 w-36 rounded-(--radius) border border-border bg-background p-1.5"
                      />
                    </If>

                    <View class="w-full gap-1.5 rounded-(--radius) border border-border bg-card p-3">
                      <Text testID="license-order-code" class="font-mono text-xs text-foreground">
                        {`Mã đơn: ${licenseTransaction()?.order_code ?? ""}`}
                      </Text>
                      <Text class="text-xs font-semibold text-success">
                        {`Số tiền: ${formatVnd(licenseTransaction()?.amount ?? 0)}`}
                      </Text>
                      <View class="flex-row items-center justify-between gap-2 pt-1">
                        <Text testID="license-transfer-memo" class="font-mono text-xs text-foreground">
                          {`Nội dung CK: ${transferMemoFor(licenseTransaction()?.order_code ?? 0)}`}
                        </Text>
                        <Button
                          testID="license-copy-memo"
                          size="sm"
                          variant="secondary"
                          label={licenseCopiedMemo() ? "Đã sao chép" : "Sao chép"}
                          onPress={() => {
                            const code = licenseTransaction()?.order_code;
                            if (code !== undefined) void copyPaymentMemo(code);
                          }}
                        />
                      </View>
                    </View>

                    <If when={licensePolling()}>
                      <Text testID="license-payment-polling" class="text-xs font-medium text-primary">
                        Đang chờ xác nhận thanh toán tự động…
                      </Text>
                    </If>

                    <Button
                      testID="license-cancel-payment"
                      size="sm"
                      variant="destructive"
                      label="Huỷ đơn & chọn gói khác"
                      onPress={resetLicensePayment}
                    />
                  </View>
                </If>
              </View>
            </If>

            {/* TAB: AUTH */}
            <If when={licenseTab() === "auth"}>
              <View class="gap-3">
                <If when={licenseUser() !== null}>
                  <View class="flex-row items-center justify-between gap-3 rounded-(--radius) border border-border bg-muted/30 p-3">
                    <View class="gap-0.5">
                      <Text class="text-xs text-muted-foreground">Đang đăng nhập với:</Text>
                      <Text testID="license-user-email" class="text-sm font-semibold text-foreground">
                        {licenseUser()?.email ?? ""}
                      </Text>
                    </View>
                    <Button
                      testID="license-logout"
                      size="sm"
                      variant="destructive"
                      label="Đăng xuất"
                      disabled={licenseLoading()}
                      onPress={() => { void logoutLicense(); }}
                    />
                  </View>
                </If>

                <If when={licenseUser() === null}>
                  <View class="gap-3">
                    <Text class="text-sm font-semibold text-foreground">
                      {isRegisterMode() ? "Đăng ký tài khoản mới" : "Đăng nhập tài khoản"}
                    </Text>
                    <View class="gap-1">
                      <Text class="text-xs text-muted-foreground">Email</Text>
                      <Input
                        testID="license-auth-email"
                        value={authEmail()}
                        placeholder="name@example.com"
                        onValue={(v) => authEmail.set(v)}
                        onSubmit={() => { void submitAuth(); }}
                      />
                    </View>
                    <View class="gap-1">
                      <Text class="text-xs text-muted-foreground">Mật khẩu</Text>
                      <Input
                        testID="license-auth-password"
                        value={authPassword()}
                        placeholder="••••••••"
                        masked
                        onValue={(v) => authPassword.set(v)}
                        onSubmit={() => { void submitAuth(); }}
                      />
                    </View>
                    <Button
                      testID="license-auth-submit"
                      variant="primary"
                      label={isRegisterMode() ? "Đăng ký" : "Đăng nhập"}
                      disabled={licenseLoading() || !licenseConfigured()}
                      onPress={() => { void submitAuth(); }}
                    />
                    <View class="items-center">
                      <Button
                        testID="license-auth-toggle"
                        size="sm"
                        variant="ghost"
                        label={
                          isRegisterMode()
                            ? "Đã có tài khoản? Đăng nhập"
                            : "Chưa có tài khoản? Đăng ký ngay"
                        }
                        onPress={() => isRegisterMode.set(!isRegisterMode())}
                      />
                    </View>
                  </View>
                </If>
              </View>
            </If>
          </View>
        </Scroll>
      </View>
    </Modal>
  );
}
