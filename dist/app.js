// Tauri API
const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;

// State
let accounts = [];
let totpTimers = new Map();

// ==================== 初始化 ====================

async function init() {
    // Windows Hello 验证（前端侧检查）
    try {
        const verified = await invoke('verify_windows_hello');
        if (!verified) {
            showErrorOverlay("Windows Hello 未配置或验证失败。请设置 PIN、指纹或面部识别后重试。");
            return;
        }
    } catch (error) {
        console.error("Windows Hello error:", error);
        showErrorOverlay(error.message || "Windows Hello 验证失败。程序即将退出。");
        // 延迟退出，让用户看到错误
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
        accounts = await invoke('load_accounts');
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

    // Start TOTP refresh for each account
    accounts.forEach(account => {
        startTOTPRefresh(account);
    });
}

async function startTOTPRefresh(account) {
    const totpEl = document.querySelector(`.account-totp[data-secret="${account.secret}"]`);
    if (!totpEl) return;

    // 首次刷新
    await refreshSingleTOTP(account, totpEl);

    // 每秒刷新
    totpTimers.set(account.id, setInterval(() => {
        refreshSingleTOTP(account, totpEl).catch(console.error);
    }, 1000));
}

async function refreshSingleTOTP(account, element) {
    try {
        const result = await invoke('generate_totp', { secret: account.secret });

        if (element && !document.contains(element)) {
            // Element may have been removed
            return;
        }

        // 格式化 TOTP（每两位加冒号）
        const formattedCode = result.code.match(/.{1,2}/g).join(':');

        if (element) {
            element.textContent = formattedCode;
            element.title = result.code; // 原始码用于 hover
        }

        // Update progress bar
        updateProgressBar(element?.parentElement, result.remaining_seconds);
    } catch (error) {
        console.error("Failed to generate TOTP:", error);
        if (element) {
            element.textContent = "ERROR";
        }
    }
}

function updateProgressBar(card, remainingSeconds) {
    const bar = card?.querySelector('.progress-bar-fill');
    if (bar && remainingSeconds !== undefined) {
        // 30 秒周期，计算百分比
        const percentage = ((remainingSeconds || 30) / 30) * 100;
        bar.style.width = `${percentage}%`;
    }
}

// ==================== 复制到剪贴板 ====================

async function copyTOTP(element) {
    try {
        // Get raw code from title (set in refreshSingleTOTP)
        const rawCode = element.title || element.textContent.replace(/:/g, '');
        await navigator.clipboard.writeText(rawCode);
        showToast("✓ 已复制到剪贴板", 1500);
    } catch (error) {
        console.error("Failed to copy:", error);
        // Fallback: select text
        element.select();
        document.execCommand('copy');
        showToast("✓ 已复制", 1500);
    }
}

function showToast(message, duration = 1500) {
    const toast = document.getElementById('copyToast');
    toast.textContent = message;
    toast.classList.add('show');

    // Clear existing timeout if any
    if (toast._timeout) {
        clearTimeout(toast._timeout);
    }

    toast._timeout = setTimeout(() => {
        toast.classList.remove('show');
    }, duration);
}

// ==================== 添加账户 ====================

function showAddAccount() {
    document.getElementById('mainView').classList.add('hidden');
    document.getElementById('addAccountView').classList.remove('hidden');
    selectTab('manual'); // Default to manual tab
}

function showMain() {
    document.getElementById('addAccountView').classList.add('hidden');
    document.getElementById('mainView').classList.remove('hidden');

    // Stop camera if running
    stopCamera();
}

// Tab switching
const tabs = document.querySelectorAll('.tab');
tabs.forEach(tab => {
    tab.addEventListener('click', () => {
        const tabName = tab.dataset.tab;
        selectTab(tabName);
    });
});

function selectTab(tabName) {
    // Update active tab button
    tabs.forEach(t => t.classList.toggle('active', t.dataset.tab === tabName));

    // Show corresponding content
    document.querySelectorAll('.tab-content').forEach(content => {
        content.classList.toggle('hidden', !content.id.endsWith(tabName + 'Form'));
    });
}

// Manual form submission
document.getElementById('manualForm')?.addEventListener('submit', async (e) => {
    e.preventDefault();

    const name = document.getElementById('accountName').value.trim();
    const issuer = document.getElementById('issuer').value.trim() || null;
    const secret = document.getElementById('secretKey').value.trim();

    if (!name || !secret) {
        showError("请输入账户名称和密钥");
        return;
    }

    try {
        const newAccount = await invoke('add_account', { name, issuer, secret });

        // Validate by generating TOTP
        await invoke('generate_totp', { secret: newAccount.secret });

        accounts.push(newAccount);
        startTOTPRefresh(newAccount);

        // Save to disk
        await invoke('save_accounts', { accounts });

        // Re-render and return to main
        renderAccountList();
        showMain();
        hideError();

        // Reset form
        document.getElementById('manualForm').reset();
    } catch (error) {
        console.error("Failed to add account:", error);
        showError(error.message || "添加账户失败");
    }
});

// Image upload handler using Tauri dialog
async function importFromImage() {
    try {
        // Use Tauri's dialog API to open file picker
        const result = await window.__TAURI__.dialog.open({
            directory: false,
            multiple: false,
            filters: [{
                name: 'Images',
                extensions: ['png', 'jpg', 'jpeg', 'bmp']
            }]
        });

        if (typeof result !== 'string') {
            return; // User cancelled
        }

        const filePath = result;

        // Parse QR code from image using Rust backend
        const otpauthUri = await invoke('parse_qr_image', { path: filePath });

        if (!otpauthUri.startsWith('otpauth://')) {
            showError("无效的二维码内容：不是 otpauth URI");
            return;
        }

        // Parse the otpauth URI into an account
        const account = await invoke('parse_otpauth_uri', { uri: otpauthUri });

        // Validate by generating TOTP
        await invoke('generate_totp', { secret: account.secret });

        accounts.push(account);
        startTOTPRefresh(account);
        await invoke('save_accounts', { accounts });

        renderAccountList();
        showMain();
        hideError();
        showToast("✓ 账户添加成功", 2000);

    } catch (error) {
        console.error("Failed to import from image:", error);
        showError(error.message || "无法解析二维码图片。请确保是有效的 otpauth:// 二维码。");
    }
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

        // Start scanning
        scanInterval = setInterval(scanFrame, 300); // Scan every 300ms

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

        // Set canvas size to match video
        canvas.width = video.videoWidth;
        canvas.height = video.videoHeight;
        ctx.drawImage(video, 0, 0);

        // Get image data
        const imageData = ctx.getImageData(0, 0, canvas.width, canvas.height);

        // Use jsQR to scan
        const code = window.jsQR(imageData, { inversionAttempts: 'dontInvert' });

        if (code) {
            console.log("QR Code found:", code.data);
            stopCamera();
            await processScannedQR(code.data);
        }
    } catch (error) {
        // Silently ignore scan errors
    }
}

async function processScannedQR(data) {
    try {
        if (!data.startsWith('otpauth://')) {
            showError("扫描的二维码不是有效的 otpauth URI");
            return;
        }

        const account = await invoke('parse_otpauth_uri', { uri: data });

        // Validate by generating TOTP
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

// Add delete button functionality via event delegation
document.addEventListener('click', async (e) => {
    if (e.target.classList.contains('delete-btn')) {
        const card = e.target.closest('.account-card');
        const id = card.dataset.id;

        // Confirm deletion
        if (!confirm("确定要删除这个账户吗？此操作无法撤销。")) {
            return;
        }

        try {
            await invoke('delete_account', { id });

            // Remove from state and stop timer
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
