import { Toast } from "@base-ui/react/toast";
import { Check, X } from "lucide-react";

import styles from "./toasts.module.css";

export const Toasts = () => {
  const { toasts } = Toast.useToastManager();

  return (
    <Toast.Portal>
      <Toast.Viewport className={styles.viewport}>
        {toasts.map((toast) => (
          <Toast.Root key={toast.id} toast={toast} className={styles.toast} swipeDirection="down">
            {toast.type === "error" ? <X size={16} /> : <Check size={16} />}
            <Toast.Title className={styles.title} render={<p />} />
          </Toast.Root>
        ))}
      </Toast.Viewport>
    </Toast.Portal>
  );
};
