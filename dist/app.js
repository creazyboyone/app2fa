// ==================== 调试日志（同时显示在页面上）====================

const debugOutput = document.getElementById('debugOutput');

function log(...args) {
    const msg = args.map(a => typeof a === 'string' ? a : JSON.stringify(a)).join(' ');
    console.log(msg);
    if (debugOutput) {
        const line = document.createElement('div');
        line.textContent = '[' + new Date().toLocaleTimeString() + '] ' + msg;
        debugOutput.appendChild(line);
        debugOutput.scrollTop = debugOutput.scrollHeight;
    }
}

log('[APP] app.js loaded');
log('[APP] window.__TAURI__ exists:', !!window.__TAURI__);

// ==================== Tauri invoke ====================
let invoke;
try {
    log('[APP] window.__TAURI__ =', JSON.stringify(window.__TAURI__));
    if (typeof window.__TAURI__ !== 'undefined') {
        const tauriCore = window.__TAURI__.core;
        log('[APP] window.__TAURI__.core =', tauriCore);
        if (tauriCore && typeof tauriCore.invoke === 'function') {
            invoke = tauriCore.invoke;
            log('[APP] Tauri invoke bound successfully');
        } else {
            throw new Error('Tauri core not available or missing invoke method');
        }
    } else {
        throw new Error('window.__TAURI__ is undefined');
    }
} catch (e) {
    log('[APP] Failed to bind Tauri invoke:', e);
    invoke = async () => {
        throw new Error('Tauri not initialized');
    };
}

// ==================== 状态管理 ====================
let accounts = [];
let totpTimers = new Map();

// ==================== 初始化 ===========================

async function init() {
    log('[INIT] TOTP Manager initializing...');

    // Windows Hello 验证（前端侧检查）
    try {
        log('[INIT] Calling verify_windows_hello...');
        const verified = await invoke('verify_windows_hello');
        if (!verified) {
            showErrorOverlay("Windows Hello 未配置或验证失败。请设置 PIN、指纹或面部识别后重试。");
            return;
        }
    } catch (error) {
        console.error("Windows Hello error:", error);
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
    log('[LOAD] Calling load_accounts...');
    try {
        let loadedAccounts = await invoke('load_accounts');
        log('[LOAD] Received', loadedAccounts.length, 'accounts from backend:', JSON.stringify(loadedAccounts));
        // Filter out invalid accounts (empty or short secrets)
        accounts = loadedAccounts.filter(a => {
            const valid = a && a.secret && a.secret.length >= 8;
            if (!valid) log('[LOAD] Filtering invalid account:', a?.name, 'secret len:', a?.secret?.length);
            return valid;
        });
        log('[LOAD] Loaded', accounts.length, 'valid accounts');
        renderAccountList();
    } catch (error) {
        console.error("Failed to load accounts:", error);
        alert("无法加载账户列表：" + error.message);
    }
}

function renderAccountList() {
    const container = document.getElementById('accountList');

    if (accounts.length === 0) {
        container.innerHTML = '<p class="hint">暂无账户。点击右上角添加。</p>';
        return;
    }

    container.innerHTML = accounts.map(account => `
        <div class="account-card" data-id="${account.id}">
            <span class="account-name">
                ${escapeHtml(account.name)}
                ${account.issuer ? `<small style="color:#565f89"> (${escapeHtml(account.issuer)})</small>` : ''}
            </span>
            <div class="account-totp" data-secret="${account.secret}" onclick="copyTOTP(this)">
                --:--:--
            </div>
            <div class="progress-bar-container">
                <div class="progress-bar-fill" style="width: 0%"></div>
            </div>
            <button class="delete-btn" title="删除账户">🗑️</button>
        </div>
    `).join('');

    accounts.forEach(account => {
        startTOTPRefresh(account);
    });
}

async function startTOTPRefresh(account) {
    if (!account?.secret || account.secret.length < 8) {
        log('[TOTP] Skipping invalid account:', account?.name);
        return;
    }
    const totpEl = document.querySelector(`.account-totp[data-secret="${account.secret}"]`);
    if (!totpEl) {
        log('[TOTP] No DOM element found for account, skipping');
        return;  // Don't proceed if no element exists
    }

    await refreshSingleTOTP(account, totpEl);

    await refreshSingleTOTP(account, totpEl);

    totpTimers.set(account.id, setInterval(() => {
        refreshSingleTOTP(account, totpEl).catch(console.error);
    }, 1000));
}

async function refreshSingleTOTP(account, element) {
    try {
        const result = await invoke('generate_totp', { secret: account.secret });

        if (element && !document.contains(element)) return;

        const formattedCode = result.code.match(/.{1,2}/g).join(':');

        if (element) {
            element.textContent = formattedCode;
            element.title = result.code;
        }

        updateProgressBar(element?.parentElement, result.remaining_seconds);
    } catch (error) {
        console.error("Failed to generate TOTP:", error);
        if (element) element.textContent = "ERROR";
    }
}

function updateProgressBar(card, remainingSeconds) {
    const bar = card?.querySelector('.progress-bar-fill');
    if (bar && remainingSeconds !== undefined) {
        const percentage = ((remainingSeconds || 30) / 30) * 100;
        bar.style.width = `${percentage}%`;
    }
}

// ==================== 复制到剪贴板 ====================

async function copyTOTP(element) {
    try {
        const rawCode = element.title || element.textContent.replace(/:/g, '');
        await navigator.clipboard.writeText(rawCode);
        showToast("✓ 已复制到剪贴板", 1500);
    } catch (error) {
        console.error("Failed to copy:", error);
        element.select();
        document.execCommand('copy');
        showToast("✓ 已复制", 1500);
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

// Define and expose immediately for onclick handlers
window.showAddAccount = function() {
    log('[APP] showAddAccount called!');
    const mainView = document.getElementById('mainView');
    const addAccountView = document.getElementById('addAccountView');
    log('[APP] mainView element:', !!mainView);
    log('[APP] addAccountView element:', !!addAccountView);
    if (mainView) { mainView.classList.add('hidden'); log('[APP] mainView hidden'); }
    if (addAccountView) { addAccountView.classList.remove('hidden'); log('[APP] addAccountView shown'); }
    selectTab('manual');
};

window.showMain = function() {
    log('[APP] showMain called');
    const addAccountView = document.getElementById('addAccountView');
    const mainView = document.getElementById('mainView');
    if (addAccountView) { addAccountView.classList.add('hidden'); }
    if (mainView) { mainView.classList.remove('hidden'); }
    stopCamera();
};

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

    log('[FORM] 表单提交被触发');
    log('[FORM] invoke function exists:', typeof invoke === 'function');

    const name = document.getElementById('accountName').value.trim();
    const issuer = document.getElementById('issuer').value.trim() || null;
    const secret = document.getElementById('secretKey').value.trim();

    if (!name) {
        showError("请输入账户名称");
        return;
    }

    // First check for invalid characters BEFORE filtering
    if (/[^A-Za-z2-7\s-]/.test(secret)) {
        showError("密钥包含无效字符。Base32 密钥只允许 A-Z、a-z、2-7、空格和横线");
        return;
    }

    const cleanedSecret = secret.replace(/[^A-Z2-7]/gi, '');
    if (cleanedSecret.length < 8) {
        showError("密钥无效。Base32 密钥通常至少 8 个字符，只包含 A-Z 和 2-7");
        return;
    }

    log('[FORM] cleanedSecret:', cleanedSecret, 'length:', cleanedSecret.length);

    log('[FORM] 准备调用 add_account:', { name, issuer, secret: cleanedSecret });

    try {
        log('[FORM] Calling invoke(add_account)...');
        const newAccount = await invoke('add_account', { name, issuer, secret: cleanedSecret });
        log('[FORM] add_account result:', JSON.stringify(newAccount));

        // Validate the returned account before using it
        if (!newAccount || !newAccount.secret || newAccount.secret.length < 8) {
            throw new Error('后端返回的账户数据无效');
        }

        log('[FORM] Calling invoke(generate_totp)...');
        await invoke('generate_totp', { secret: newAccount.secret });

        accounts.push(newAccount);
        startTOTPRefresh(newAccount);

        log('[FORM] Calling invoke(save_accounts)...');
        await invoke('save_accounts', { accounts });

        renderAccountList();
        showMain();
        hideError();
        document.getElementById('manualForm').reset();
        log('[FORM] 账户添加成功');
    } catch (error) {
        log('[FORM] Error adding account:', error);
        console.error("Failed to add account:", error);
        showError(error.message || "添加账户失败");
    }
});

// ==================== 图片导入（占位符）=====================

async function importFromImage() {
    showError("QR 图片导入功能正在开发中，请使用手动输入或摄像头扫描");
}

document.getElementById('qrFileInputBtn')?.addEventListener('click', importFromImage);

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
        console.error("Camera error:", error);
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
        const code = window.jsQR(imageData, { inversionAttempts: 'dontInvert' });

        if (code) {
            console.log("QR Code found:", code.data);
            stopCamera();
            await processScannedQR(code.data);
        }
    } catch (error) {}
}

async function processScannedQR(data) {
    try {
        if (!data.startsWith('otpauth://')) {
            showError("扫描的二维码不是有效的 otpauth URI");
            return;
        }

        const account = await invoke('parse_otpauth_uri', { uri: data });
        await invoke('generate_totp', { secret: account.secret });

        accounts.push(account);
        startTOTPRefresh(account);
        await invoke('save_accounts', { accounts });

        renderAccountList();
        showMain();
        hideError();
        showToast("✓ 账户添加成功", 2000);
    } catch (error) {
        console.error("Failed to process QR:", error);
        showError(error.message || "无法解析二维码内容");
    }
}

// ==================== 删除账户 ====================

document.addEventListener('click', async (e) => {
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
            console.error("Failed to delete:", error);
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
