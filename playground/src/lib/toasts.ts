import { Toast } from "@base-ui/react/toast";

export const toastManager = Toast.createToastManager();

let shown: string | undefined;

export const notify = (title: string, type?: "error") => {
  const previous = shown;
  shown = toastManager.add({ title, type });
  if (previous !== undefined) toastManager.close(previous);
};
