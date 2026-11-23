// A test-only entry: the toast layer with a button that raises a toast with a retry.
import { mount, signal, Text, View } from "nsh";
import { Button, Toast } from "../ui";
import { notify } from "../stores/toast";

function Harness() {
  const runs = signal(0);
  return (
    <View class="flex-1 bg-background p-6">
      <Button testID="fire" label="Báo lỗi" onPress={() => notify("Dịch vụ chưa sẵn sàng", "danger", () => runs.set(runs() + 1))} />
      <Text testID="runs">{`retry chạy ${runs()} lần`}</Text>
      <Toast />
    </View>
  );
}
mount(Harness);
