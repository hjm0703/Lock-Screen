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

async function loadBackgroundImages(): Promise<void> {
  try {
    const images = await invoke<string[]>("list_background_images");
    const selectEl = document.getElementById("setting-bg-image-file") as HTMLSelectElement;
    if (!selectEl) return;
    // 保留当前选中值
    const currentVal = selectEl.value;
    selectEl.innerHTML = "";
    if (images.length === 0) {
      const opt = document.createElement("option");
      opt.value = "";
      opt.textContent = "暂无图片";
      selectEl.appendChild(opt);
    } else {
      const emptyOpt = document.createElement("option");
      emptyOpt.value = "";
      emptyOpt.textContent = "（不选择）";
      selectEl.appendChild(emptyOpt);
      images.forEach((name) => {
        const opt = document.createElement("option");
        opt.value = name;
        opt.textContent = name;
        selectEl.appendChild(opt);
      });
    }
    // 恢复选中值
    if (currentVal && images.includes(currentVal)) {
      selectEl.value = currentVal;
    }
  } catch {
    // ignore
  }
}

function updateBgImageUI(enabled: boolean): void {
  const selectItem = document.getElementById("bg-image-select-item");
  const dimmedItem = document.getElementById("bg-image-show-dimmed-item");
  const overlayItem = document.getElementById("bg-image-show-overlay-item");
  const opacityOverlayItem = document.getElementById("bg-image-opacity-overlay-item");
  const opacityDimmedItem = document.getElementById("bg-image-opacity-dimmed-item");
  const display = enabled ? "flex" : "none";
  if (selectItem) selectItem.style.display = display;
  if (dimmedItem) dimmedItem.style.display = display;
  if (overlayItem) overlayItem.style.display = display;
  if (opacityOverlayItem) opacityOverlayItem.style.display = display;
  if (opacityDimmedItem) opacityDimmedItem.style.display = display;
  if (enabled) {
    loadBackgroundImages();
  }
}

async function loadSettings(): Promise<void> {
  try {
    const result = await invoke("get_settings");
    const settings = result as Record<string, unknown>;
    const autoHideEl = document.getElementById("setting-auto-hide") as HTMLInputElement;
    const breathingEl = document.getElementById("setting-breathing-light") as HTMLInputElement;
    const clockVisibleEl = document.getElementById("setting-clock-visible") as HTMLInputElement;
    const bgImageEnabledEl = document.getElementById("setting-bg-image-enabled") as HTMLInputElement;
    const bgImageShowDimmedEl = document.getElementById("setting-bg-image-show-dimmed") as HTMLInputElement;
    const bgImageShowOverlayEl = document.getElementById("setting-bg-image-show-overlay") as HTMLInputElement;
    const bgImageOpacityOverlayEl = document.getElementById("setting-bg-image-opacity-overlay") as HTMLInputElement;
    const bgImageOpacityOverlayValEl = document.getElementById("value-bg-image-opacity-overlay");
    const bgImageOpacityDimmedEl = document.getElementById("setting-bg-image-opacity-dimmed") as HTMLInputElement;
    const bgImageOpacityDimmedValEl = document.getElementById("value-bg-image-opacity-dimmed");
    const bgImageFileEl = document.getElementById("setting-bg-image-file") as HTMLSelectElement;

    if (autoHideEl) autoHideEl.checked = Boolean(settings.auto_hide);
    if (breathingEl) breathingEl.checked = Boolean(settings.breathing_light);
    if (clockVisibleEl) clockVisibleEl.checked = Boolean(settings.clock_visible ?? true);
    if (bgImageEnabledEl) {
      const enabled = Boolean(settings.bg_image_enabled);
      bgImageEnabledEl.checked = enabled;
      updateBgImageUI(enabled);
    }
    if (bgImageShowDimmedEl) bgImageShowDimmedEl.checked = Boolean(settings.bg_image_show_dimmed);
    if (bgImageShowOverlayEl) bgImageShowOverlayEl.checked = Boolean(settings.bg_image_show_overlay ?? true);
    if (bgImageOpacityOverlayEl) {
      const stored = (settings.bg_image_opacity_overlay as number) ?? 1.0;
      const v = Math.round((1 - stored) * 100);
      bgImageOpacityOverlayEl.value = String(v);
      if (bgImageOpacityOverlayValEl) bgImageOpacityOverlayValEl.textContent = `${v}%`;
    }
    if (bgImageOpacityDimmedEl) {
      const stored = (settings.bg_image_opacity_dimmed as number) ?? 1.0;
      const v = Math.round((1 - stored) * 100);
      bgImageOpacityDimmedEl.value = String(v);
      if (bgImageOpacityDimmedValEl) bgImageOpacityDimmedValEl.textContent = `${v}%`;
    }
    if (bgImageFileEl) {
      const file = settings.bg_image_file as string | null;
      if (file) bgImageFileEl.value = file;
    }
  } catch {
    // ignore load errors
  }
}

const NEED_RESTART_KEYS = [
  "breathing_light",
  "bg_image_enabled",
  "bg_image_show_dimmed",
  "bg_image_show_overlay",
  "bg_image_opacity_overlay",
  "bg_image_opacity_dimmed",
  "clock_visible",
];

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

  // 时钟可见性
  const clockVisibleEl = document.getElementById("setting-clock-visible");
  if (clockVisibleEl) {
    clockVisibleEl.addEventListener("change", (e) => {
      handleToggleSetting("clock_visible", (e.target as HTMLInputElement).checked);
    });
  }

  // 背景图片开关
  const bgImageEnabledEl = document.getElementById("setting-bg-image-enabled");
  if (bgImageEnabledEl) {
    bgImageEnabledEl.addEventListener("change", (e) => {
      const checked = (e.target as HTMLInputElement).checked;
      handleToggleSetting("bg_image_enabled", checked);
      updateBgImageUI(checked);
    });
  }

  // 隐藏密码框时显示背景
  const bgImageShowDimmedEl = document.getElementById("setting-bg-image-show-dimmed");
  if (bgImageShowDimmedEl) {
    bgImageShowDimmedEl.addEventListener("change", (e) => {
      handleToggleSetting("bg_image_show_dimmed", (e.target as HTMLInputElement).checked);
    });
  }

  // 显示密码框时显示背景
  const bgImageShowOverlayEl = document.getElementById("setting-bg-image-show-overlay");
  if (bgImageShowOverlayEl) {
    bgImageShowOverlayEl.addEventListener("change", (e) => {
      handleToggleSetting("bg_image_show_overlay", (e.target as HTMLInputElement).checked);
    });
  }

  // 选择背景图片
  const bgImageFileEl = document.getElementById("setting-bg-image-file");
  if (bgImageFileEl) {
    bgImageFileEl.addEventListener("change", (e) => {
      const val = (e.target as HTMLSelectElement).value || null;
      invoke("set_bg_image_file", { filename: val }).catch(() => {});
    });
  }

  // 背景图片透明度 - 显示密码框时（滑块值越大越透明）
  const bgImageOpacityOverlayEl = document.getElementById("setting-bg-image-opacity-overlay") as HTMLInputElement;
  const bgImageOpacityOverlayValEl = document.getElementById("value-bg-image-opacity-overlay");
  if (bgImageOpacityOverlayEl) {
    bgImageOpacityOverlayEl.addEventListener("input", (e) => {
      const val = parseInt((e.target as HTMLInputElement).value, 10);
      if (bgImageOpacityOverlayValEl) bgImageOpacityOverlayValEl.textContent = `${val}%`;
      handleSliderChange("bg_image_opacity_overlay", 100 - val);
    });
  }

  // 背景图片透明度 - 隐藏密码框时（滑块值越大越透明）
  const bgImageOpacityDimmedEl = document.getElementById("setting-bg-image-opacity-dimmed") as HTMLInputElement;
  const bgImageOpacityDimmedValEl = document.getElementById("value-bg-image-opacity-dimmed");
  if (bgImageOpacityDimmedEl) {
    bgImageOpacityDimmedEl.addEventListener("input", (e) => {
      const val = parseInt((e.target as HTMLInputElement).value, 10);
      if (bgImageOpacityDimmedValEl) bgImageOpacityDimmedValEl.textContent = `${val}%`;
      handleSliderChange("bg_image_opacity_dimmed", 100 - val);
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

  // 导入图片按钮
  const importBtn = document.getElementById("btn-import-wallpaper");
  const fileInput = document.getElementById("wallpaper-file-input") as HTMLInputElement;
  if (importBtn && fileInput) {
    importBtn.addEventListener("click", () => {
      fileInput.click();
    });
    fileInput.addEventListener("change", async () => {
      const file = fileInput.files?.[0];
      if (!file) return;
      try {
        // 通过 Rust 后端复制到 EXE/images 目录
        const fileName = file.name;
        const bytes = Array.from(new Uint8Array(await file.arrayBuffer()));
        await invoke("import_wallpaper", { fileName, bytes });
        // 刷新列表并选中
        await loadBackgroundImages();
        const selectEl = document.getElementById("setting-bg-image-file") as HTMLSelectElement;
        if (selectEl && Array.from(selectEl.options).some((o) => o.value === fileName)) {
          selectEl.value = fileName;
          await invoke("set_bg_image_file", { filename: fileName });
        }
        fileInput.value = "";
      } catch (err: unknown) {
        console.error("导入图片失败:", err);
      }
    });
  }

});