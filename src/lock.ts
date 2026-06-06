import { invoke } from "@tauri-apps/api/core";

let passwordVisible = true;
let password = "";
let clickPollTimer: ReturnType<typeof setInterval> | null = null;

async function initLockScreen(): Promise<void> {
  const container = document.getElementById("password-container");
  const overlay = document.getElementById("lock-overlay");
  if (!container || !overlay) return;

  // 初始状态：密码框隐藏，overlay 显示 dimmed
  container.classList.add("hidden");
  overlay.classList.add("dimmed");

  // 开始轮询鼠标点击
  startClickPolling();
}

function startClickPolling(): void {
  const hint = document.getElementById("click-hint");
  if (!hint) return;

  clickPollTimer = setInterval(async () => {
    try {
      const clicked = await invoke<boolean>("poll_mouse_click");
      if (clicked) {
        hint.classList.remove("visible");
        void hint.offsetWidth; // 强制回流，重置动画
        hint.classList.add("visible");
      }
    } catch (_) {
      // 忽略轮询错误
    }
  }, 200);
}

function stopClickPolling(): void {
  if (clickPollTimer !== null) {
    clearInterval(clickPollTimer);
    clickPollTimer = null;
  }
}

function showMessage(text: string): void {
  const el = document.getElementById("unlock-message");
  if (el) el.textContent = text;
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

async function togglePasswordVisibility(): Promise<void> {
  passwordVisible = !passwordVisible;
  const container = document.querySelector(".lock-container") as HTMLElement;
  const overlay = document.getElementById("lock-overlay");
  if (!container || !overlay) return;

  if (passwordVisible) {
    container.classList.remove("hidden");
    overlay.classList.remove("dimmed");
    const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
    if (pwdEl) pwdEl.focus();
    await invoke("set_password_visible", { visible: true });
  } else {
    container.classList.add("hidden");
    overlay.classList.add("dimmed");
    await invoke("set_password_visible", { visible: false });
  }
}

async function handleUnlock(): Promise<void> {
  const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
  const pwd = pwdEl?.value || "";

  if (!pwd) {
    showMessage("请输入密码");
    return;
  }

  try {
    const valid = await invoke("verify_password", { password: pwd }) as boolean;
    if (valid) {
      showMessage("");
      if (pwdEl) pwdEl.value = "";
      await invoke("unlock_screen");
    } else {
      showMessage("密码错误");
      if (pwdEl) pwdEl.value = "";
    }
  } catch (err: unknown) {
    showMessage(`解锁失败: ${err as string}`);
  }
}

window.addEventListener("DOMContentLoaded", () => {
  applySettings();
  initLockScreen();

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
      void togglePasswordVisibility();
    }
  });
});
