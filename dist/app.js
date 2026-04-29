// ==================== Tauri invoke ====================
let invoke;
try {
    if (typeof window.__TAURI__ !== 'undefined') {
        const tauriCore = window.__TAURI__.core;
        if (tauriCore && typeof tauriCore.invoke === 'function') {
            invoke = tauriCore.invoke;
        } else {
            throw new Error('Tauri core not available or missing invoke method');
        }
    } else {
        throw new Error('window.__TAURI__ is undefined');
    }
} catch (e) {
    invoke = async () => {
        throw new Error('Tauri not initialized');
    };
}

// ==================== 状态管理 ====================
let accounts = [];
let totpTimers = new Map();

// ==================== 初始化 ===========================

async function init() {
    try {
        const verified = await invoke('verify_windows_hello');
        if (!verified) {
            showErrorOverlay("Windows Hello 未配置或验证失败。请设置 PIN、指纹或面部识别后重试。");
            return;
        }
    } catch (error) {
        showErrorOverlay(error.message || "Windows Hello 验证失败。程序即将退出。");
        setTimeout(() => window.close(), 2000);
        return;
    }

    await loadAndRenderAccounts();
    hideLoading();
}

function hideLoading() {
    document.getElementById('loadingOverlay').classList.add('hidden');
}

function showErrorOverlay(message) {
    const overlay = document.getElementById('helloErrorOverlay');
    overlay.querySelector('h2').textContent = '⚠️ Windows Hello 验证失败';
    overlay.querySelectorAll('p')[0].textContent = message;
    overlay.classList.remove('hidden');
}

// ==================== 账户加载与渲染 ====================

async function loadAndRenderAccounts() {
    try {
        let loadedAccounts = await invoke('load_accounts');
        accounts = loadedAccounts.filter(a => a && a.secret && a.secret.length >= 8);
        sortAccounts();
        renderAccountList();
    } catch (error) {
        alert("无法加载账户列表：" + error.message);
    }
}

function sortAccounts() {
    accounts.sort((a, b) => {
        // 置顶优先
        if (a.pinned !== b.pinned) return b.pinned ? 1 : -1;
        // 使用次数多的优先
        if ((b.usage_count || 0) !== (a.usage_count || 0)) {
            return (b.usage_count || 0) - (a.usage_count || 0);
        }
        // 最近使用的优先
        const aTime = a.last_used_at || 0;
        const bTime = b.last_used_at || 0;
        return bTime - aTime;
    });
}

function renderAccountList() {
    const container = document.getElementById('accountList');

    if (accounts.length === 0) {
        container.innerHTML = '<p class="hint">暂无账户。点击右上角添加。</p>';
        return;
    }

    container.innerHTML = accounts.map(account => `
        <div class="account-card${account.pinned ? ' pinned' : ''}" data-id="${account.id}">
            <button class="pin-btn${account.pinned ? ' pinned' : ''}" title="${account.pinned ? '取消置顶' : '置顶'}">📌</button>
            <div class="account-info">
                ${account.issuer ? `<div class="account-issuer">${escapeHtml(account.issuer)}</div>` : ''}
                <div class="account-name">${escapeHtml(account.name)}</div>
            </div>
            <div class="account-totp" data-secret="${account.secret}">
                <span class="totp-progress"></span>
                <span class="totp-text">------</span>
            </div>
            <button class="delete-btn" title="删除">×</button>
        </div>
    `).join('');

    // 为 TOTP 元素绑定点击事件
    container.querySelectorAll('.account-totp').forEach(el => {
        el.addEventListener('click', () => copyTOTP(el));
    });

    // 滚动条显示/隐藏
    let scrollTimeout;
    container.addEventListener('scroll', () => {
        container.classList.add('scrolling');
        clearTimeout(scrollTimeout);
        scrollTimeout = setTimeout(() => {
            container.classList.remove('scrolling');
        }, 1000);
    });

    accounts.forEach(account => {
        startTOTPRefresh(account);
    });
}

async function startTOTPRefresh(account) {
    if (!account?.secret || account.secret.length < 8) return;
    const totpEl = document.querySelector(`.account-totp[data-secret="${account.secret}"]`);
    if (!totpEl) return;

    await refreshSingleTOTP(account, totpEl);

    totpTimers.set(account.id, setInterval(() => {
        refreshSingleTOTP(account, totpEl).catch(() => {});
    }, 1000));
}

async function refreshSingleTOTP(account, element) {
    try {
        const result = await invoke('generate_totp', { secret: account.secret });

        if (element && !document.contains(element)) return;

        const textEl = element?.querySelector('.totp-text');
        const progressEl = element?.querySelector('.totp-progress');

        if (textEl) {
            textEl.textContent = result.code;
        }
        if (element) {
            element.title = result.code;
        }

        // Update progress fill
        if (progressEl) {
            const percentage = ((result.remaining_seconds || 30) / 30) * 100;
            progressEl.style.width = `${percentage}%`;
        }
    } catch (error) {
        const textEl = element?.querySelector('.totp-text');
        if (textEl) textEl.textContent = "ERROR";
    }
}

// ==================== 复制到剪贴板 ====================

async function copyTOTP(element) {
    const rawCode = element.title || element.querySelector('.totp-text')?.textContent || '';
    const card = element.closest('.account-card');
    const accountId = card?.dataset.id;

    try {
        await invoke('copy_to_clipboard', { text: rawCode });
        // 更新使用记录
        if (accountId) {
            try {
                await invoke('update_account_usage', { id: accountId });
                // 更新本地账户数据
                const account = accounts.find(a => a.id === accountId);
                if (account) {
                    account.usage_count = (account.usage_count || 0) + 1;
                    account.last_used_at = Math.floor(Date.now() / 1000);
                }
            } catch (e) {}
        }
        // 显示系统通知
        await invoke('show_notification', { title: "2FA Manager", body: "验证码已复制到剪贴板，已最小化到托盘" });
        // 最小化到托盘
        await invoke('minimize_to_tray');
    } catch (e) {
        console.error('Error:', e);
    }
}


function showToast(message, duration = 1500) {
    const toast = document.getElementById('copyToast');
    toast.textContent = message;
    toast.classList.add('show');
    if (toast._timeout) clearTimeout(toast._timeout);
    toast._timeout = setTimeout(() => toast.classList.remove('show'), duration);
}

// ==================== 添加账户 ====================

function showAddAccount() {
    const mainView = document.getElementById('mainView');
    const addAccountView = document.getElementById('addAccountView');
    if (mainView) mainView.classList.add('hidden');
    if (addAccountView) addAccountView.classList.remove('hidden');
    selectTab('manual');
}

function showMain() {
    const addAccountView = document.getElementById('addAccountView');
    const mainView = document.getElementById('mainView');
    if (addAccountView) addAccountView.classList.add('hidden');
    if (mainView) mainView.classList.remove('hidden');
    stopCamera();
}

// 绑定按钮事件
document.getElementById('btnAddAccount')?.addEventListener('click', showAddAccount);
document.getElementById('btnCloseAddAccount')?.addEventListener('click', showMain);
document.getElementById('btnStartCamera')?.addEventListener('click', startCamera);
document.getElementById('btnCloseApp')?.addEventListener('click', () => window.close());

const tabs = document.querySelectorAll('.tab');
tabs.forEach(tab => {
    tab.addEventListener('click', () => selectTab(tab.dataset.tab));
});

function selectTab(tabName) {
    tabs.forEach(t => t.classList.toggle('active', t.dataset.tab === tabName));
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.toggle('hidden', !content.id.endsWith(tabName + 'Form'));
    });
}

document.getElementById('manualForm')?.addEventListener('submit', async (e) => {
    e.preventDefault();

    const name = document.getElementById('accountName').value.trim();
    const issuer = document.getElementById('issuer').value.trim() || null;
    const secret = document.getElementById('secretKey').value.trim();

    if (!name) {
        showError("请输入账户名称");
        return;
    }

    if (/[^A-Za-z2-7\s-]/.test(secret)) {
        showError("密钥包含无效字符。Base32 密钥只允许 A-Z、a-z、2-7、空格和横线");
        return;
    }

    const cleanedSecret = secret.replace(/[^A-Z2-7]/gi, '');
    if (cleanedSecret.length < 8) {
        showError("密钥无效。Base32 密钥通常至少 8 个字符，只包含 A-Z 和 2-7");
        return;
    }

    // 去重检查
    if (isAccountExists(cleanedSecret)) {
        showError("该账户已存在");
        return;
    }

    try {
        const newAccount = await invoke('add_account', { name, issuer, secret: cleanedSecret });

        if (!newAccount || !newAccount.secret || newAccount.secret.length < 8) {
            throw new Error('后端返回的账户数据无效');
        }

        await invoke('generate_totp', { secret: newAccount.secret });

        accounts.push(newAccount);
        startTOTPRefresh(newAccount);

        await invoke('save_accounts', { accounts });

        renderAccountList();
        showMain();
        hideError();
        document.getElementById('manualForm').reset();
    } catch (error) {
        showError(error.message || "添加账户失败");
    }
});

// ==================== 图片导入 ====================

async function importFromImage() {
    try {
        const input = document.getElementById('qrFileInput');
        if (!input || !input.files || input.files.length === 0) return;

        const files = Array.from(input.files);
        const allowedTypes = ['image/png', 'image/jpeg', 'image/jpg', 'image/bmp', 'image/gif'];
        let importedCount = 0;
        let skippedCount = 0;
        let failedCount = 0;

        for (const file of files) {
            if (!allowedTypes.includes(file.type)) {
                failedCount++;
                continue;
            }

            try {
                const dataUrl = await new Promise((resolve, reject) => {
                    const reader = new FileReader();
                    reader.onload = () => resolve(reader.result);
                    reader.onerror = reject;
                    reader.readAsDataURL(file);
                });

                const img = new Image();
                await new Promise((resolve, reject) => {
                    img.onload = resolve;
                    img.onerror = reject;
                    img.src = dataUrl;
                });

                const MAX_SIZE = 2048;
                let w = img.width, h = img.height;
                if (w > MAX_SIZE || h > MAX_SIZE) {
                    const scale = MAX_SIZE / Math.max(w, h);
                    w = Math.round(w * scale);
                    h = Math.round(h * scale);
                }

                if (w < 10 || h < 10) {
                    failedCount++;
                    continue;
                }

                const canvas = document.getElementById('qrCanvas');
                canvas.width = w;
                canvas.height = h;
                const ctx = canvas.getContext('2d');
                ctx.drawImage(img, 0, 0, w, h);

                const imageData = ctx.getImageData(0, 0, w, h);

                let code = null;
                try {
                    code = window.jsQR(imageData.data, imageData.width, imageData.height, {
                        inversionAttempts: 'attemptBoth'
                    });
                } catch (e) {}

                let qrData = code?.data;
                if (!qrData) {
                    try {
                        qrData = await invoke('parse_qr_image', { dataUrl });
                    } catch (e) {}
                }

                if (qrData) {
                    const result = await processQRData(qrData, false);
                    importedCount += result.imported;
                    skippedCount += result.skipped;
                    if (result.imported === 0 && result.skipped === 0) {
                        failedCount++;
                    }
                } else {
                    failedCount++;
                }
            } catch (e) {
                failedCount++;
            }
        }

        // 保存所有账户
        if (importedCount > 0) {
            await invoke('save_accounts', { accounts });
        }

        // 显示结果
        if (importedCount > 0) {
            renderAccountList();
            showMain();
            hideError();
            let msg = `✓ 成功导入 ${importedCount} 个账户`;
            if (skippedCount > 0) msg += `，${skippedCount} 个已存在跳过`;
            if (failedCount > 0) msg += `，${failedCount} 个失败`;
            showToast(msg, 2500);
        } else if (skippedCount > 0) {
            showToast(`所有账户已存在，跳过 ${skippedCount} 个`, 2000);
        } else {
            showError("未能识别任何有效的二维码");
        }

        // 重置文件输入
        input.value = '';
    } catch (error) {
        showError(error.message || "读取图片失败");
    }
}

document.getElementById('qrFileInputBtn')?.addEventListener('click', () => {
    document.getElementById('qrFileInput')?.click();
});
document.getElementById('qrFileInput')?.addEventListener('change', importFromImage);

// ==================== 摄像头扫描 ====================

let cameraStream = null;
let scanInterval = null;

async function startCamera() {
    try {
        const video = document.getElementById('cameraVideo');
        cameraStream = await navigator.mediaDevices.getUserMedia({
            video: { facingMode: 'environment' }
        });
        video.srcObject = cameraStream;
        video.style.display = 'block';
        scanInterval = setInterval(scanFrame, 300);
    } catch (error) {
        showError("无法访问摄像头：" + error.message);
    }
}

function stopCamera() {
    if (cameraStream) {
        cameraStream.getTracks().forEach(track => track.stop());
        cameraStream = null;
    }
    if (scanInterval) {
        clearInterval(scanInterval);
        scanInterval = null;
    }
    const video = document.getElementById('cameraVideo');
    if (video) {
        video.srcObject = null;
        video.style.display = 'none';
    }
}

async function scanFrame() {
    try {
        const video = document.getElementById('cameraVideo');
        const canvas = document.getElementById('qrCanvas');
        const ctx = canvas.getContext('2d');

        if (!video.srcObject || !video.videoWidth) return;

        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        ctx.drawImage(video, 0, 0);
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);
        const code = window.jsQR(imageData.data, imageData.width, imageData.height, {
            inversionAttempts: 'dontInvert'
        });

        if (code) {
            stopCamera();
            await processScannedQR(code.data);
        }
    } catch (error) {}
}

// 检查账户是否已存在（基于 secret 去重）
function isAccountExists(secret) {
    return accounts.some(a => a.secret === secret);
}

// 处理 QR 数据，返回 { imported: number, skipped: number }
async function processQRData(data, autoSave = true) {
    try {
        // Google Authenticator 迁移
        if (data.startsWith('otpauth-migration://offline?data=')) {
            let migrationData = data.substring('otpauth-migration://offline?data='.length);
            const migrationAccounts = await invoke('decode_migration_payload', { b64Data: migrationData });

            if (migrationAccounts.length === 0) return { imported: 0, skipped: 0 };

            let imported = 0, skipped = 0;
            for (const ma of migrationAccounts) {
                // 去重检查
                if (isAccountExists(ma.secret_b32)) {
                    skipped++;
                    continue;
                }

                try {
                    const account = await invoke('add_account', { name: ma.name, issuer: ma.issuer || null, secret: ma.secret_b32 });
                    accounts.push(account);
                    startTOTPRefresh(account);
                    imported++;
                } catch (e) {}
            }

            if (imported > 0 && autoSave) {
                await invoke('save_accounts', { accounts });
            }
            return { imported, skipped };
        }

        // 标准 otpauth URI
        if (!data.startsWith('otpauth://')) {
            return { imported: 0, skipped: 0 };
        }

        const account = await invoke('parse_otpauth_uri', { uri: data });

        // 去重检查
        if (isAccountExists(account.secret)) {
            return { imported: 0, skipped: 1 };
        }

        accounts.push(account);
        startTOTPRefresh(account);

        if (autoSave) {
            await invoke('save_accounts', { accounts });
        }
        return { imported: 1, skipped: 0 };
    } catch (error) {
        return { imported: 0, skipped: 0 };
    }
}

async function processScannedQR(data) {
    const result = await processQRData(data, true);
    if (result.imported > 0) {
        renderAccountList();
        showMain();
        hideError();
        showToast(result.imported > 1 ? `✓ 成功导入 ${result.imported} 个账户` : "✓ 账户添加成功", 2000);
    } else if (result.skipped > 0) {
        showToast("该账户已存在", 2000);
    } else {
        showError("无法解析二维码内容");
    }
}

// ==================== 删除账户 ====================

document.addEventListener('click', async (e) => {
    // 置顶按钮
    if (e.target.classList.contains('pin-btn')) {
        const card = e.target.closest('.account-card');
        const id = card?.dataset.id;
        if (!id) return;

        try {
            await invoke('toggle_pin', { id });
            const account = accounts.find(a => a.id === id);
            if (account) {
                account.pinned = !account.pinned;
            }
            sortAccounts();
            renderAccountList();
        } catch (error) {
            alert("操作失败：" + error.message);
        }
        return;
    }

    // 删除按钮
    if (e.target.classList.contains('delete-btn')) {
        const card = e.target.closest('.account-card');
        const id = card.dataset.id;

        if (!confirm("确定要删除这个账户吗？此操作无法撤销。")) return;

        try {
            await invoke('delete_account', { id });
            accounts = accounts.filter(a => a.id !== id);
            const timer = totpTimers.get(id);
            if (timer) {
                clearInterval(timer);
                totpTimers.delete(id);
            }
            renderAccountList();
        } catch (error) {
            alert("删除失败：" + error.message);
        }
    }
});

// ==================== 工具函数 ====================

function escapeHtml(text) {
    const div = document.createElement('div');
    div.textContent = text;
    return div.innerHTML;
}

function showError(message) {
    const errorEl = document.getElementById('errorMsg');
    errorEl.textContent = message;
    errorEl.classList.remove('hidden');
}

function hideError() {
    document.getElementById('errorMsg').classList.add('hidden');
}

// ==================== 启动应用 ====================

window.addEventListener('DOMContentLoaded', init);
