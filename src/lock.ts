import { invoke } from "@tauri-apps/api/core";

let passwordVisible = true;

// 星期映射
const WEEK_DAYS = ["日", "一", "二", "三", "四", "五", "六"];

function updateClock(): void {
  const now = new Date();
  const hours = String(now.getHours()).padStart(2, "0");
  const minutes = String(now.getMinutes()).padStart(2, "0");
  const timeEl = document.getElementById("clock-time");
  if (timeEl) timeEl.textContent = `${hours}:${minutes}`;

  const year = now.getFullYear();
  const month = now.getMonth() + 1;
  const day = now.getDate();
  const weekDay = WEEK_DAYS[now.getDay()];
  const dateEl = document.getElementById("clock-date");
  if (dateEl) dateEl.textContent = `${year}年${month}月${day}日 星期${weekDay}`;
}

async function initLockScreen(): Promise<void> {
  const container = document.querySelector(".lock-form-section") as HTMLElement;
  const overlay = document.getElementById("lock-overlay");
  if (!container || !overlay) return;

  // 初始状态：密码框隐藏，overlay 显示 dimmed
  container.classList.add("hidden");
  overlay.classList.add("dimmed");

  // 根据 settings 切换背景图片的 dimmed/overlay 状态
  updateBgImageVisibility(false);

  // 开始更新时钟
  updateClock();
  setInterval(updateClock, 1000);

  // 开始轮询鼠标点击
  startClickPolling();
}

function updateBgImageVisibility(isPasswordVisible: boolean): void {
  const bgImg = document.getElementById("lock-bg-img") as HTMLImageElement;
  if (!bgImg || !bgImg.src) return;

  const showDimmed = (window as unknown as Record<string, boolean>).__bgImageShowDimmed ?? false;
  const showOverlay = (window as unknown as Record<string, boolean>).__bgImageShowOverlay ?? true;

  if (isPasswordVisible) {
    bgImg.classList.toggle("show", showOverlay);
  } else {
    bgImg.classList.toggle("show", showDimmed);
  }
}

function startClickPolling(): void {
  const hint = document.getElementById("click-hint");
  if (!hint) return;

  setInterval(async () => {
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

function showMessage(text: string): void {
  const el = document.getElementById("unlock-message");
  if (el) el.textContent = text;
}

function applySettings(): void {
  const overlay = document.getElementById("lock-overlay");
  if (!overlay) return;

  const breathingLight = (window as unknown as Record<string, boolean>).__breathingLight ?? true;
  const clockVisible = (window as unknown as Record<string, boolean>).__clockVisible ?? true;
  const bgImageUrl = (window as unknown as Record<string, string | null>).__bgImageUrl ?? null;
  const bgImageOpacityOverlay = (window as unknown as Record<string, number>).__bgImageOpacityOverlay ?? 1;
  const bgImageOpacityDimmed = (window as unknown as Record<string, number>).__bgImageOpacityDimmed ?? 1;

  if (breathingLight) {
    overlay.classList.add("breathing");
  } else {
    overlay.classList.remove("breathing");
  }

  // 时钟显示/隐藏
  if (clockVisible) {
    overlay.classList.remove("clock-hidden");
  } else {
    overlay.classList.add("clock-hidden");
  }

  // 背景图片
  const bgImg = document.getElementById("lock-bg-img") as HTMLImageElement;
  if (bgImg && bgImageUrl) {
    bgImg.src = bgImageUrl;
    overlay.classList.add("has-bg-image");
    bgImg.style.setProperty("--bg-image-opacity-overlay", String(bgImageOpacityOverlay));
    bgImg.style.setProperty("--bg-image-opacity-dimmed", String(bgImageOpacityDimmed));
  } else {
    overlay.classList.remove("has-bg-image");
  }
}

async function togglePasswordVisibility(): Promise<void> {
  passwordVisible = !passwordVisible;
  const container = document.querySelector(".lock-form-section") as HTMLElement;
  const overlay = document.getElementById("lock-overlay");
  if (!container || !overlay) return;

  if (passwordVisible) {
    container.classList.remove("hidden");
    overlay.classList.remove("dimmed");
    updateBgImageVisibility(true);
    const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
    if (pwdEl) pwdEl.focus();
    await invoke("set_password_visible", { visible: true });
  } else {
    container.classList.add("hidden");
    overlay.classList.add("dimmed");
    updateBgImageVisibility(false);
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