import { ask } from "@tauri-apps/plugin-dialog";

/** Asks before something that changes or deletes data. `false` when the person says no or the dialog fails. */
export async function confirmed(message: string, title = "Please confirm", ok = "Continue"): Promise<boolean> {
  try {
    return await ask(message, { title, kind: "warning", okLabel: ok, cancelLabel: "Cancel" });
  } catch {
    return false;
  }
}
