/**
 * Grafyx - Interactive Background
 * A lightweight node-graph visualization for the landing page.
 */

class GrafyxBackground {
    constructor() {
        this.canvas = document.getElementById('background-canvas');
        if (!this.canvas) return;
        this.ctx = this.canvas.getContext('2d');
        this.nodes = [];
        this.nodeCount = 80;
        this.maxDistance = 200;
        this.mouse = { x: null, y: null };
        
        this.init();
        this.animate();
        this.addEventListeners();
    }

    init() {
        this.resize();
        this.nodes = [];
        for (let i = 0; i < this.nodeCount; i++) {
            this.nodes.push({
                x: Math.random() * this.canvas.width,
                y: Math.random() * this.canvas.height,
                vx: (Math.random() - 0.5) * 0.5,
                vy: (Math.random() - 0.5) * 0.5,
                radius: Math.random() * 2 + 1
            });
        }
    }

    resize() {
        this.canvas.width = window.innerWidth;
        this.canvas.height = window.innerHeight;
    }

    draw() {
        this.ctx.clearRect(0, 0, this.canvas.width, this.canvas.height);
        
        for (let i = 0; i < this.nodes.length; i++) {
            let n = this.nodes[i];
            
            // Move
            n.x += n.vx;
            n.y += n.vy;
            
            // Bounce
            if (n.x < 0 || n.x > this.canvas.width) n.vx *= -1;
            if (n.y < 0 || n.y > this.canvas.height) n.vy *= -1;
            
            // Draw Node
            this.ctx.beginPath();
            this.ctx.arc(n.x, n.y, n.radius, 0, Math.PI * 2);
            this.ctx.fillStyle = 'rgba(100, 255, 180, 0.5)';
            this.ctx.fill();
            
            // Connections
            for (let j = i + 1; j < this.nodes.length; j++) {
                let n2 = this.nodes[j];
                const dx = n.x - n2.x;
                const dy = n.y - n2.y;
                const dist = Math.sqrt(dx*dx + dy*dy);
                
                if (dist < this.maxDistance) {
                    this.ctx.beginPath();
                    this.ctx.moveTo(n.x, n.y);
                    this.ctx.lineTo(n2.x, n2.y);
                    const alpha = (1 - dist / this.maxDistance) * 0.2;
                    this.ctx.strokeStyle = `rgba(100, 255, 180, ${alpha})`;
                    this.ctx.stroke();
                }
            }
        }
    }

    animate() {
        this.draw();
        requestAnimationFrame(() => this.animate());
    }

    addEventListeners() {
        window.addEventListener('resize', () => this.resize());
        window.addEventListener('mousemove', (e) => {
            this.mouse.x = e.clientX;
            this.mouse.y = e.clientY;

            // Apply repulsion force to nodes
            this.nodes.forEach(node => {
                const dx = node.x - this.mouse.x;
                const dy = node.y - this.mouse.y;
                const dist = Math.sqrt(dx*dx + dy*dy);
                if (dist < 150) {
                    if (dist === 0) return; // Skip if directly on node
                    const force = (150 - dist) / 150;
                    node.vx += dx / dist * force * 0.8;
                    node.vy += dy / dist * force * 0.8;
                }
            });
        });
        window.addEventListener('mouseleave', () => {
            this.mouse.x = null;
            this.mouse.y = null;
        });
    }
}


// Terminal Simulation Logic
const terminalSteps = [
    { text: "Establishing secure context...", delay: 800 },
    { text: "[INIT] Initializing deep BFS scanner...", delay: 400 },
    { text: "[SEC] Verifying Ed25519 signatures...", delay: 600 },
    { text: "[SEC] Root of Trust confirmed.", delay: 300 },
    { text: "PARSING: Found 142 distinct modules.", delay: 600 },
    { text: "LINKING: Resolving cross-references...", delay: 1000 },
    { text: "MAP: Topological sort complete.", delay: 400 },
    { text: "[SUCCESS] Graph generated in 8ms.", delay: 800, class: "success" },
    { text: "Streaming to http://localhost:8080", delay: 1000 }
];

async function runTerminalSim() {
    const output = document.getElementById('terminal-output');
    if (!output) return;

    for (const step of terminalSteps) {
        const line = document.createElement('div');
        line.className = 'line animate-fade-in';
        if (step.class) line.classList.add(step.class);
        line.innerHTML = `<span class="time">[${new Date().toLocaleTimeString([], {hour12: false})}]</span> ${step.text}`;
        output.appendChild(line);
        
        // Auto-scroll
        const body = document.getElementById('terminal-body');
        body.scrollTop = body.scrollHeight;
        
        await new Promise(r => setTimeout(r, step.delay));
    }
}

// Magnetic Buttons
function magneticButtons() {
    document.querySelectorAll('.btn').forEach(btn => {
        btn.addEventListener('mouseenter', () => {
            btn.style.transition = 'background 0.3s, box-shadow 0.3s';
        });
        btn.addEventListener('mousemove', (e) => {
            const rect = btn.getBoundingClientRect();
            const x = e.clientX - rect.left - rect.width / 2;
            const y = e.clientY - rect.top - rect.height / 2;
            btn.style.transform = `translate(${x * 0.2}px, ${y * 0.2}px)`;
        });
        btn.addEventListener('mouseleave', () => {
            btn.style.transition = '';
            btn.style.transform = ``;
        });
    });
}

class GrafyxInstaller {
    constructor() {
        this.platformButtons = document.querySelectorAll('.platform-btn');
        this.cliLines = document.querySelector('#cli-box');
        this.repo = '0xarchit/grafyx';
        this.releaseData = null;
        this.platforms = {
            linux: {
                assetMatch: 'linux-amd64-static',
                cmd: (url) => [
                    `curl -L ${url} -o grafyx`,
                    `chmod +x grafyx && ./grafyx install`
                ]
            },
            macos: {
                assetMatch: 'macos-aarch64', // Default to ARM
                cmd: (url) => [
                    `curl -L ${url} -o grafyx`,
                    `chmod +x grafyx && ./grafyx install`
                ]
            },
            windows: {
                assetMatch: 'windows-amd64.exe',
                cmd: (url) => [
                    `iwr ${url} -OutFile grafyx.exe`,
                    `.\\grafyx install`
                ]
            }
        };

        this.init();
    }

    async init() {
        if (!this.platformButtons.length) return;
        
        await Promise.all([
            this.fetchRelease(),
            this.fetchRepoStats()
        ]);
        
        this.detectOS();
        this.addEventListeners();
        this.updateUI();
    }

    async fetchRepoStats() {
        const el = document.getElementById('gh-star-count');
        try {
            const response = await fetch(`https://api.github.com/repos/${this.repo}`);
            if (!response.ok) throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
            const data = await response.json();
            const count = data.stargazers_count;
            const formatted = count > 999 ? (count / 1000).toFixed(1) + 'k' : count;
            if (el) el.innerText = formatted;
        } catch (e) {
            if (el) el.innerText = '0';
        }
    }

    async fetchRelease() {
        try {
            const response = await fetch(`https://api.github.com/repos/${this.repo}/releases/latest`);
            if (!response.ok) throw new Error(`GitHub API error: ${response.status} ${response.statusText}`);
            this.releaseData = await response.json();
        } catch (e) {
            console.warn('GitHub API failed, using fallback URLs');
            // Fallback to generic tag URLs if API fails
            this.releaseData = {
                assets: [
                    { name: 'grafyx-linux-amd64-static', browser_download_url: `https://github.com/${this.repo}/releases/latest/download/grafyx-linux-amd64-static` },
                    { name: 'grafyx-macos-aarch64', browser_download_url: `https://github.com/${this.repo}/releases/latest/download/grafyx-macos-aarch64` },
                    { name: 'grafyx-macos-x86_64', browser_download_url: `https://github.com/${this.repo}/releases/latest/download/grafyx-macos-x86_64` },
                    { name: 'grafyx-windows-amd64.exe', browser_download_url: `https://github.com/${this.repo}/releases/latest/download/grafyx-windows-amd64.exe` }
                ]
            };
        }
    }

    detectOS() {
        const platform = window.navigator.platform.toLowerCase();
        let detected = 'linux';
        
        if (platform.includes('win')) {
            detected = 'windows';
        } else if (platform.includes('mac')) {
            detected = 'macos';
            this.platforms.macos.assetMatch = 'macos-x86_64'; // Default to x86
            if (navigator.userAgentData && navigator.userAgentData.getHighEntropyValues) {
                navigator.userAgentData.getHighEntropyValues(['architecture']).then(values => {
                    if (values.architecture === 'arm' || values.architecture === 'arm64') {
                        this.platforms.macos.assetMatch = 'macos-aarch64';
                    }
                    this.updateUI();
                });
            }
        }
        
        this.setActive(detected);
    }

    setActive(os) {
        this.platformButtons.forEach(btn => {
            btn.classList.toggle('active', btn.dataset.os === os);
        });
        this.currentOS = os;
        this.updateUI();
    }

    addEventListeners() {
        this.platformButtons.forEach(btn => {
            btn.addEventListener('click', () => this.setActive(btn.dataset.os));
        });
    }

    updateUI() {
        if (!this.releaseData || !this.cliLines) return;
        
        const config = this.platforms[this.currentOS];
        const asset = this.releaseData.assets.find(a => 
            a.name.endsWith(config.assetMatch) && !a.name.endsWith('.sig')
        );
        
        if (!asset) {
            this.cliLines.innerHTML = '<div class="cli-line" style="color: var(--accent); opacity: 0.8;">Download currently unavailable for this architecture. Please check releases manually.</div>';
            return;
        }

        const commands = config.cmd(asset.browser_download_url);
        
        this.cliLines.innerHTML = commands.map((c, i) => 
            `<div class="cli-line" ${i === commands.length - 1 ? 'style="margin-bottom: 0;"' : ''}>${c}</div>`
        ).join('');
    }
}

// Clipboard
async function copyCLI() {
    try {
        const lines = document.querySelectorAll('.cli-line');
        const text = Array.from(lines).map(l => l.innerText.trim()).join('\n');
        await navigator.clipboard.writeText(text);
        
        const btn = document.querySelector('.copy-btn');
        const icon = btn ? btn.querySelector('i') : null;
        
        if (btn && icon && window.lucide) {
            icon.setAttribute('data-lucide', 'check');
            window.lucide.createIcons();
            
            setTimeout(() => {
                const stillIcon = btn.querySelector('i');
                if (stillIcon) {
                    stillIcon.setAttribute('data-lucide', 'copy');
                    window.lucide.createIcons();
                }
            }, 2000);
        }
    } catch (err) {
        console.error('Failed to copy: ', err);
        alert('Failed to copy to clipboard. Please copy manually.');
    }
}

// Custom Cursor Logic
function customCursor() {
    const dot = document.querySelector('.cursor-dot');
    const outline = document.querySelector('.cursor-outline');
    if (!dot || !outline) return;

    let animationFrameId = null;
    let targetX = 0, targetY = 0;

    const animateOutline = () => {
        outline.style.left = `${targetX}px`;
        outline.style.top = `${targetY}px`;
        animationFrameId = null;
    };

    window.addEventListener('mousemove', (e) => {
        dot.style.left = `${e.clientX}px`;
        dot.style.top = `${e.clientY}px`;
        
        targetX = e.clientX;
        targetY = e.clientY;
        
        if (!animationFrameId) {
            animationFrameId = requestAnimationFrame(animateOutline);
        }
    });

    document.body.style.cursor = 'none';

    document.querySelectorAll('a, button, .card').forEach(el => {
        el.addEventListener('mouseenter', () => {
            outline.style.transform = 'translate(-50%, -50%) scale(2)';
            outline.style.background = 'rgba(100, 255, 180, 0.1)';
        });
        el.addEventListener('mouseleave', () => {
            outline.style.transform = 'translate(-50%, -50%) scale(1)';
            outline.style.background = 'var(--accent-glow)';
        });
    });
}

// Initialize on Load
document.addEventListener('DOMContentLoaded', () => {
    new GrafyxBackground();
    new GrafyxInstaller();
    magneticButtons();
    customCursor();
    
    // Initialize Lucide Icons
    if (window.lucide) {
        window.lucide.createIcons();
    }
    

    // Run terminal sim when visible
    const termObserver = new IntersectionObserver((entries) => {
        if (entries[0].isIntersecting) {
            runTerminalSim();
            termObserver.disconnect();
        }
    }, { threshold: 0.5 });
    
    const term = document.querySelector('.terminal-sim');
    if (term) termObserver.observe(term);

    // Scroll reveals
    const observerOptions = {
        threshold: 0.1
    };

    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.style.opacity = '1';
                entry.target.style.transform = 'translateY(0)';
            }
        });
    }, observerOptions);

    document.querySelectorAll('.animate-on-scroll').forEach(el => {
        el.style.opacity = '0';
        el.style.transform = 'translateY(30px)';
        el.style.transition = 'all 0.8s cubic-bezier(0.16, 1, 0.3, 1)';
        observer.observe(el);
    });
});
