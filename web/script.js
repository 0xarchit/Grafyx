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
        });
        window.addEventListener('mouseleave', () => {
            this.mouse.x = null;
            this.mouse.y = null;
        });
    }
}


// Terminal Simulation Logic
const terminalSteps = [
    { text: "Initializing Rust analyzer...", delay: 500 },
    { text: "Scanning 1,024 source files...", delay: 800 },
    { text: "Building dependency adjacency list...", delay: 1200 },
    { text: "Calculating D3 Force-Directed layout...", delay: 1000 },
    { text: "Success: Discovered 4,281 connections.", delay: 500, class: "success" },
    { text: "Opening http://localhost:8000 ...", delay: 800 }
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
        btn.addEventListener('mousemove', (e) => {
            const rect = btn.getBoundingClientRect();
            const x = e.clientX - rect.left - rect.width / 2;
            const y = e.clientY - rect.top - rect.height / 2;
            btn.style.transform = `translate(${x * 0.2}px, ${y * 0.2}px)`;
        });
        btn.addEventListener('mouseleave', () => {
            btn.style.transform = ``;
        });
    });
}

// Clipboard
function copyCLI() {
    const lines = document.querySelectorAll('.cli-line');
    const text = Array.from(lines).map(l => l.innerText.trim()).join('\n');
    navigator.clipboard.writeText(text);
    
    const btn = document.querySelector('.copy-btn');
    const icon = btn.querySelector('i');
    
    if (window.lucide) {
        icon.setAttribute('data-lucide', 'check');
        window.lucide.createIcons();
        
        setTimeout(() => {
            icon.setAttribute('data-lucide', 'copy');
            window.lucide.createIcons();
        }, 2000);
    }
}

// Custom Cursor Logic
function customCursor() {
    const dot = document.querySelector('.cursor-dot');
    const outline = document.querySelector('.cursor-outline');
    if (!dot || !outline) return;

    window.addEventListener('mousemove', (e) => {
        dot.style.left = `${e.clientX}px`;
        dot.style.top = `${e.clientY}px`;
        
        outline.animate({
            left: `${e.clientX}px`,
            top: `${e.clientY}px`
        }, { duration: 500, fill: "forwards" });
    });

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
