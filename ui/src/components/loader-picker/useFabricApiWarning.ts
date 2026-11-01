// 加载器已装好但 Fabric API 安装失败时（`loader:fabric-api-failed`）弹出提醒，
// 引导用户到实例的资源页重新安装。须在 NDialogProvider 内、组件 setup 中调用。
import { useDialog } from "naive-ui";
import { onBeforeUnmount } from "vue";
import { useI18n } from "vue-i18n";
import { useRouter } from "vue-router";
import { onFabricApiFailed } from "../../api/events";
import { formatError } from "../../i18n";
import { fabricApiRetryRoute } from "./loaders";

/** 订阅事件并在组件卸载时取消；没有后端时什么也不做。 */
export function useFabricApiWarning() {
  const { t } = useI18n();
  const dialog = useDialog();
  const router = useRouter();
  let unlisten: (() => void) | null = null;
  let unmounted = false;

  onFabricApiFailed(({ instance_id, instance_name, error }) => {
    dialog.warning({
      title: t("loaderPicker.fabricApiFailed.title"),
      content: t("loaderPicker.fabricApiFailed.content", {
        name: instance_name,
        error: formatError(error),
      }),
      positiveText: t("loaderPicker.fabricApiFailed.open"),
      negativeText: t("loaderPicker.fabricApiFailed.later"),
      onPositiveClick: () => {
        void router.push(fabricApiRetryRoute(instance_id));
      },
    });
  })
    .then((fn) => {
      if (unmounted) fn();
      else unlisten = fn;
    })
    .catch(() => {
      // 在浏览器中调试时没有后端。
    });

  onBeforeUnmount(() => {
    unmounted = true;
    unlisten?.();
  });
}
