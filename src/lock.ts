import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

let passwordVisible = true;

function showMessage(el: HTMLElement | null, text: string): void {
  if (!el) return;
  el.textContent = text;
}

function applySettings(): void {
  const overlay = document.getElementById("lock-overlay");
  if (!overlay) return;

  const overlayOpacity = (window as unknown as Record<string, number>).__overlayOpacity ?? 0.55;
  const dimmedOpacity = (window as unknown as Record<string, number>).__dimmedOpacity ?? 0.85;
  const breathingLight = (window as unknown as Record<string, boolean>).__breathingLight ?? true;

  overlay.style.setProperty("--overlay-opacity", String(overlayOpacity));
  overlay.style.setProperty("--dimmed-opacity", String(dimmedOpacity));

  if (breathingLight) {
    overlay.classList.add("breathing");
  } else {
    overlay.classList.remove("breathing");
  }
}

function togglePasswordVisibility(): void {
  passwordVisible = !passwordVisible;
  const container = document.querySelector(".lock-container") as HTMLElement;
  const overlay = document.getElementById("lock-overlay");
  if (!container || !overlay) return;

  if (passwordVisible) {
    container.classList.remove("hidden");
    overlay.classList.remove("dimmed");
    const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
    if (pwdEl) pwdEl.focus();
  } else {
    container.classList.add("hidden");
    overlay.classList.add("dimmed");
  }
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
  applySettings();

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

  // ESC 切换密码框显示/隐藏
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape") {
      togglePasswordVisibility();
    }
  });
});
