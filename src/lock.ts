import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

function showMessage(el: HTMLElement | null, text: string): void {
  if (!el) return;
  el.textContent = text;
}

async function handleUnlock(): Promise<void> {
  const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
  const msgEl = document.getElementById("unlock-message");
  const pwd = pwdEl?.value || "";

  if (!pwd) {
    showMessage(msgEl, "请输入密码");
    return;
  }

  try {
    const valid = await invoke("verify_password", { password: pwd }) as boolean;
    if (valid) {
      showMessage(msgEl, "");
      if (pwdEl) pwdEl.value = "";
      await invoke("unlock_screen");
    } else {
      showMessage(msgEl, "密码错误");
      if (pwdEl) pwdEl.value = "";
    }
  } catch (err: unknown) {
    showMessage(msgEl, `解锁失败: ${err as string}`);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
  const unlockBtn = document.getElementById("btn-unlock");

  if (pwdEl) {
    pwdEl.focus();
    pwdEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        void handleUnlock();
      }
    });
  }

  if (unlockBtn) {
    unlockBtn.addEventListener("click", () => {
      void handleUnlock();
    });
  }
});
