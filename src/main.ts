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

    if (autoHideEl) autoHideEl.checked = Boolean(settings.auto_hide);
  } catch {
    // ignore load errors
  }
}

function handleToggleSetting(key: string, checked: boolean): void {
  invoke("update_setting", { key, value: checked }).catch(() => {
    // ignore save errors
  });
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

  const savePasswordBtn = document.getElementById("btn-save-password");
  if (savePasswordBtn) {
    savePasswordBtn.addEventListener("click", () => {
      void handleSavePassword();
    });
  }
});
