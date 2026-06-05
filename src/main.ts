import { invoke } from "@tauri-apps/api/core";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

function navigateTo(page: string): void {
  document.querySelectorAll(".page").forEach((el) => {
    el.classList.remove("active");
  });
  document.querySelectorAll(".nav-item").forEach((el) => {
    el.classList.remove("active");
  });

  const pageEl = document.getElementById(`page-${page}`);
  const navEl = document.querySelector(`.nav-item[data-page="${page}"]`);

  if (pageEl) {
    pageEl.classList.add("active");
  }
  if (navEl) {
    navEl.classList.add("active");
  }

  if (page === "settings") {
    loadSettings();
  }
  if (page === "password") {
    updatePasswordForm().catch(() => {});
  }
  if (page === "lock") {
    updateLockPage().catch(() => {});
  }
}

function showMessage(el: HTMLElement | null, text: string, type: "success" | "error"): void {
  if (!el) return;
  el.textContent = text;
  el.className = `message ${type}`;
}

async function updatePasswordForm(): Promise<void> {
  const oldGroup = document.getElementById("old-password-group");
  const descEl = document.getElementById("password-page-desc");
  const oldPwdEl = document.getElementById("old-password") as HTMLInputElement;

  try {
    const hasPwd = await invoke("has_password") as boolean;
    if (hasPwd) {
      if (oldGroup) oldGroup.style.display = "block";
      if (descEl) descEl.textContent = "已设置密码，修改需要验证原密码";
    } else {
      if (oldGroup) oldGroup.style.display = "none";
      if (descEl) descEl.textContent = "请设置一个安全的密码来锁定您的屏幕";
    }
    if (oldPwdEl) oldPwdEl.value = "";
  } catch {
    // ignore
  }
}

async function handleSavePassword(): Promise<void> {
  const oldPwdEl = document.getElementById("old-password") as HTMLInputElement;
  const newPwdEl = document.getElementById("new-password") as HTMLInputElement;
  const confirmPwdEl = document.getElementById("confirm-password") as HTMLInputElement;
  const msgEl = document.getElementById("password-message");

  const oldPwd = oldPwdEl?.value || "";
  const newPwd = newPwdEl?.value || "";
  const confirmPwd = confirmPwdEl?.value || "";

  if (!newPwd) {
    showMessage(msgEl, "请输入密码", "error");
    return;
  }

  if (newPwd.length < 4) {
    showMessage(msgEl, "密码长度不能少于4位", "error");
    return;
  }

  if (newPwd !== confirmPwd) {
    showMessage(msgEl, "两次输入的密码不一致", "error");
    return;
  }

  try {
    const hasPwd = await invoke("has_password") as boolean;
    if (hasPwd) {
      await invoke("set_password", { password: newPwd, oldPassword: oldPwd });
    } else {
      await invoke("set_password", { password: newPwd });
    }
    showMessage(msgEl, "密码设置成功", "success");
    if (oldPwdEl) oldPwdEl.value = "";
    if (newPwdEl) newPwdEl.value = "";
    if (confirmPwdEl) confirmPwdEl.value = "";
    await updatePasswordForm();
  } catch (err: unknown) {
    showMessage(msgEl, `保存失败: ${err as string}`, "error");
  }
}

function handleWindowMinimize(): void {
  const window = getCurrentWebviewWindow();
  void window.minimize();
}

function handleWindowClose(): void {
  const window = getCurrentWebviewWindow();
  void window.hide();
}

async function loadSettings(): Promise<void> {
  try {
    const result = await invoke("get_settings");
    const settings = result as Record<string, unknown>;
    const autoHideEl = document.getElementById("setting-auto-hide") as HTMLInputElement;
    const overlayEl = document.getElementById("setting-overlay-opacity") as HTMLInputElement;
    const dimmedEl = document.getElementById("setting-dimmed-opacity") as HTMLInputElement;
    const overlayValEl = document.getElementById("value-overlay-opacity");
    const dimmedValEl = document.getElementById("value-dimmed-opacity");
    const breathingEl = document.getElementById("setting-breathing-light") as HTMLInputElement;

    if (autoHideEl) autoHideEl.checked = Boolean(settings.auto_hide);
    if (overlayEl) {
      const v = Math.round(((settings.overlay_opacity as number) ?? 0.55) * 100);
      overlayEl.value = String(v);
      if (overlayValEl) overlayValEl.textContent = `${v}%`;
    }
    if (dimmedEl) {
      const v = Math.round(((settings.dimmed_opacity as number) ?? 0.85) * 100);
      dimmedEl.value = String(v);
      if (dimmedValEl) dimmedValEl.textContent = `${v}%`;
    }
    if (breathingEl) breathingEl.checked = Boolean(settings.breathing_light);
  } catch {
    // ignore load errors
  }
}

const NEED_RESTART_KEYS = ["overlay_opacity", "dimmed_opacity", "breathing_light"];

function showRestartBanner(): void {
  const banner = document.getElementById("restart-banner");
  if (banner) banner.classList.add("active");
}

function handleToggleSetting(key: string, checked: boolean): void {
  invoke("update_setting", { key, value: checked ? 1.0 : 0.0 })
    .then(() => {
      if (NEED_RESTART_KEYS.includes(key)) {
        showRestartBanner();
      }
    })
    .catch(() => {
      // ignore save errors
    });
}

function handleSliderChange(key: string, value: number): void {
  invoke("update_setting", { key, value: value / 100 })
    .then(() => {
      if (NEED_RESTART_KEYS.includes(key)) {
        showRestartBanner();
      }
    })
    .catch(() => {
      // ignore save errors
    });
}

async function updateLockPage(): Promise<void> {
  const msgEl = document.getElementById("lock-message");
  const pwdEl = document.getElementById("lock-password") as HTMLInputElement;
  if (msgEl) {
    msgEl.textContent = "";
    msgEl.className = "message";
  }
  if (pwdEl) pwdEl.value = "";

  try {
    const hasPwd = await invoke("has_password") as boolean;
    if (!hasPwd) {
      if (msgEl) {
        msgEl.textContent = "请先设置密码后再使用锁屏功能";
        msgEl.className = "message error";
      }
    }
  } catch {
    // ignore
  }
}

async function handleStartLock(): Promise<void> {
  const pwdEl = document.getElementById("lock-password") as HTMLInputElement;
  const msgEl = document.getElementById("lock-message");
  const pwd = pwdEl?.value || "";

  if (!pwd) {
    if (msgEl) {
      msgEl.textContent = "请输入密码";
      msgEl.className = "message error";
    }
    return;
  }

  try {
    const valid = await invoke("verify_password", { password: pwd }) as boolean;
    if (valid) {
      if (msgEl) {
        msgEl.textContent = "";
        msgEl.className = "message";
      }
      if (pwdEl) pwdEl.value = "";
      await invoke("start_lock_screen");
    } else {
      if (msgEl) {
        msgEl.textContent = "密码错误";
        msgEl.className = "message error";
      }
      if (pwdEl) pwdEl.value = "";
    }
  } catch (err: unknown) {
    if (msgEl) {
      msgEl.textContent = `启动失败: ${err as string}`;
      msgEl.className = "message error";
    }
  }
}

window.addEventListener("DOMContentLoaded", () => {
  loadSettings().catch(() => {});

  document.querySelectorAll(".nav-item").forEach((item) => {
    item.addEventListener("click", () => {
      const page = item.getAttribute("data-page");
      if (page) {
        navigateTo(page);
      }
    });
  });

  document.querySelectorAll(".action-card").forEach((card) => {
    card.addEventListener("click", () => {
      const goto = card.getAttribute("data-goto");
      if (goto) {
        navigateTo(goto);
      }
    });
  });

  const minimizeBtn = document.getElementById("btn-minimize");
  if (minimizeBtn) {
    minimizeBtn.addEventListener("click", handleWindowMinimize);
  }

  const closeBtn = document.getElementById("btn-close");
  if (closeBtn) {
    closeBtn.addEventListener("click", handleWindowClose);
  }

  const autoHideEl = document.getElementById("setting-auto-hide");
  if (autoHideEl) {
    autoHideEl.addEventListener("change", (e) => {
      handleToggleSetting("auto_hide", (e.target as HTMLInputElement).checked);
    });
  }

  const breathingEl = document.getElementById("setting-breathing-light");
  if (breathingEl) {
    breathingEl.addEventListener("change", (e) => {
      handleToggleSetting("breathing_light", (e.target as HTMLInputElement).checked);
    });
  }

  const overlayEl = document.getElementById("setting-overlay-opacity") as HTMLInputElement;
  const overlayValEl = document.getElementById("value-overlay-opacity");
  if (overlayEl) {
    overlayEl.addEventListener("input", (e) => {
      const val = parseInt((e.target as HTMLInputElement).value, 10);
      if (overlayValEl) overlayValEl.textContent = `${val}%`;
      handleSliderChange("overlay_opacity", val);
    });
  }

  const dimmedEl = document.getElementById("setting-dimmed-opacity") as HTMLInputElement;
  const dimmedValEl = document.getElementById("value-dimmed-opacity");
  if (dimmedEl) {
    dimmedEl.addEventListener("input", (e) => {
      const val = parseInt((e.target as HTMLInputElement).value, 10);
      if (dimmedValEl) dimmedValEl.textContent = `${val}%`;
      handleSliderChange("dimmed_opacity", val);
    });
  }

  const savePasswordBtn = document.getElementById("btn-save-password");
  if (savePasswordBtn) {
    savePasswordBtn.addEventListener("click", () => {
      void handleSavePassword();
    });
  }

  const startLockBtn = document.getElementById("btn-start-lock");
  if (startLockBtn) {
    startLockBtn.addEventListener("click", () => {
      void handleStartLock();
    });
  }

  const lockPwdEl = document.getElementById("lock-password") as HTMLInputElement;
  if (lockPwdEl) {
    lockPwdEl.addEventListener("keydown", (e) => {
      if (e.key === "Enter") {
        void handleStartLock();
      }
    });
  }

});
