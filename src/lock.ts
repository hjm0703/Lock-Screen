import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

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

  // 保持焦点在密码输入框
  startFocusGuard();
}

function updateBgImageVisibility(_isPasswordVisible: boolean): void {
  const bgImg = document.getElementById("lock-bg-img") as HTMLImageElement;
  if (!bgImg || !bgImg.src) return;

  // 壁纸始终显示
  bgImg.classList.add("show");
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

function startFocusGuard(): void {
  const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
  if (!pwdEl) return;

  // 密码框失去焦点时，如果密码框区域可见则重新聚焦
  pwdEl.addEventListener("blur", () => {
    const container = document.querySelector(".lock-form-section");
    if (container && !container.classList.contains("hidden")) {
      setTimeout(() => pwdEl.focus(), 10);
    }
  });

  // 定时检查焦点，防止被其他方式移走
  setInterval(() => {
    const container = document.querySelector(".lock-form-section");
    if (container && !container.classList.contains("hidden") && document.activeElement !== pwdEl) {
      pwdEl.focus();
    }
  }, 300);
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
  const passwordHint = (window as unknown as Record<string, string | null>).__passwordHint ?? null;

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

  // 密码提示
  const hintEl = document.getElementById("password-hint-display");
  if (hintEl) {
    hintEl.textContent = passwordHint || "";
  }

  // 更新时间戳
  updateTimestamp();
}

function updateTimestamp(): void {
  const el = document.getElementById("lock-timestamp");
  if (!el) return;
  const ts = (window as unknown as Record<string, number>).__lockTimestamp;
  if (ts) {
    el.textContent = String(ts);
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
    updateTimestamp();
    const pwdEl = document.getElementById("unlock-password") as HTMLInputElement;
    if (pwdEl) {
      pwdEl.focus();
      void invoke("ensure_caps_lock_off");
    }
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
      const welcomeScreen = (window as unknown as Record<string, boolean>).__welcomeScreen ?? false;
      if (welcomeScreen) {
        showWelcomeScreen();
      } else {
        await invoke("unlock_screen");
      }
    } else {
      showMessage("密码错误");
      if (pwdEl) pwdEl.value = "";
      const form = document.querySelector(".lock-form") as HTMLElement | null;
      if (form) {
        form.classList.remove("shake");
        void form.offsetWidth; // 强制回流
        form.classList.add("shake");
      }
    }
  } catch (err: unknown) {
    showMessage(`解锁失败: ${err as string}`);
  }
}

function showWelcomeScreen(): void {
  const welcomeEl = document.getElementById("welcome-screen");
  if (!welcomeEl) {
    void invoke("unlock_screen");
    return;
  }
  welcomeEl.classList.remove("active");
  void welcomeEl.offsetWidth;
  welcomeEl.classList.add("active");
  setTimeout(() => {
    welcomeEl.classList.remove("active");
    void invoke("unlock_screen");
  }, 500);
}

function setupCapsLockHint(inputId: string, hintId: string): void {
  const input = document.getElementById(inputId) as HTMLInputElement;
  const hint = document.getElementById(hintId);
  if (!input || !hint) return;

  input.addEventListener("focus", () => {
    void invoke("ensure_caps_lock_off");
  });

  input.addEventListener("keydown", (e) => {
    if (e.getModifierState("CapsLock")) {
      hint.classList.add("visible");
    } else {
      hint.classList.remove("visible");
    }
  });

  input.addEventListener("keyup", (e) => {
    if (!e.getModifierState("CapsLock")) {
      hint.classList.remove("visible");
    }
  });

  input.addEventListener("blur", () => {
    hint.classList.remove("visible");
  });
}

window.addEventListener("DOMContentLoaded", () => {
  applySettings();
  initLockScreen();

  // 监听时间戳事件
  void listen<number>("lock-timestamp", (event) => {
    const ts = event.payload;
    (window as unknown as Record<string, number>).__lockTimestamp = ts;
    const el = document.getElementById("lock-timestamp");
    if (el) el.textContent = String(ts);
  });

  // 大写锁定提示
  setupCapsLockHint("unlock-password", "unlock-caps-hint");

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