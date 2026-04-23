/**
 * Grafyx - Production Scripts
 * Implements Cinematic Reveal, Live Physics Playground, GitHub API, and Magnetic UI.
 */

document.addEventListener('DOMContentLoaded', () => {
    fetchGitHubData();
    initHeroAnimations();
    initCLI();
    initBackground();
    initPlayground();
    initMagneticButtons();
    initSpotlight();
    initScrollReveal();
    initCustomCursor();
    initViewportTilt();
});

async function fetchGitHubData() {
    const repo = '0xarchit/grafyx';
    const versionEl = document.getElementById('version-tag');
    const starsEl = document.getElementById('gh-stars');

    try {
        // Fetch Stars
        const repoRes = await fetch(`https://api.github.com/repos/${repo}`);
        if (repoRes.ok) {
            const data = await repoRes.json();
            const stars = data.stargazers_count;
            if (starsEl) starsEl.innerText = stars > 999 ? (stars / 1000).toFixed(1) + 'k' : stars;
        }

        // Fetch Latest Version
        const releaseRes = await fetch(`https://api.github.com/repos/${repo}/releases/latest`);
        if (releaseRes.ok) {
            const data = await releaseRes.json();
            if (versionEl) versionEl.innerText = `${data.tag_name} Experimental`;
        } else {
            if (versionEl) versionEl.innerText = 'v0.1.4 Experimental';
        }
    } catch (err) {
        console.error('Failed to fetch GitHub data:', err);
        if (versionEl) versionEl.innerText = 'v0.1.4 Experimental';
    }
}

function initHeroAnimations() {
    const titles = document.querySelectorAll('.hero-title, .hero-description, .cli-container, .hero-actions');
    titles.forEach((el, i) => {
        setTimeout(() => {
            el.classList.add('reveal');
        }, 100 * i);
    });
}

function initCLI() {
    const tabs = document.querySelectorAll('.tab-btn');
    const display = document.getElementById('cli-display');
    
    const commands = {
        'linux': 'curl -L https://github.com/0xarchit/grafyx/releases/latest/download/grafyx-linux-amd64-static -o grafyx && chmod +x grafyx && ./grafyx install && rm grafyx',
        'macos-arm': 'curl -L https://github.com/0xarchit/grafyx/releases/latest/download/grafyx-macos-aarch64 -o grafyx && chmod +x grafyx && ./grafyx install && rm grafyx',
        'macos-intel': 'curl -L https://github.com/0xarchit/grafyx/releases/latest/download/grafyx-macos-x86_64 -o grafyx && chmod +x grafyx && ./grafyx install && rm grafyx',
        'windows': 'iwr https://github.com/0xarchit/grafyx/releases/latest/download/grafyx-windows-amd64.exe -OutFile grafyx.exe; .\\grafyx install; del grafyx.exe'
    };

    tabs.forEach(tab => {
        tab.addEventListener('click', () => {
            tabs.forEach(t => t.classList.remove('active'));
            tab.classList.add('active');
            display.innerText = commands[tab.dataset.platform];
        });
    });

    const ua = navigator.userAgent.toLowerCase();
    if (ua.includes('win')) {
        tabs[3].click();
    } else if (ua.includes('mac')) {
        if (ua.includes('intel')) {
            tabs[2].click();
        } else {
            tabs[1].click();
        }
    } else {
        tabs[0].click();
    }
}

function copyToClipboard() {
    const text = document.getElementById('cli-display').innerText;
    navigator.clipboard.writeText(text).then(() => {
        const toast = document.getElementById('toast');
        toast.classList.add('show');
        setTimeout(() => toast.classList.remove('show'), 2000);
        
        const icon = document.querySelector('.cli-copy i');
        if (icon) {
            icon.className = 'fa-solid fa-check';
            setTimeout(() => {
                icon.className = 'fa-regular fa-copy';
            }, 2000);
        }
    });
}

function initBackground() {
    const canvas = document.getElementById('background-canvas');
    const ctx = canvas.getContext('2d');
    let width, height;
    let nodes = [];
    const nodeCount = 50;
    const maxDistance = 250;

    function resize() {
        width = canvas.width = window.innerWidth;
        height = canvas.height = window.innerHeight;
    }

    function initNodes() {
        nodes = [];
        for (let i = 0; i < nodeCount; i++) {
            nodes.push({
                x: Math.random() * width,
                y: Math.random() * height,
                vx: (Math.random() - 0.5) * 0.3,
                vy: (Math.random() - 0.5) * 0.3,
                r: Math.random() * 2 + 1
            });
        }
    }

    function animate() {
        ctx.clearRect(0, 0, width, height);
        nodes.forEach((n, i) => {
            n.x += n.vx;
            n.y += n.vy;
            if (n.x < 0 || n.x > width) n.vx *= -1;
            if (n.y < 0 || n.y > height) n.vy *= -1;
            ctx.beginPath();
            ctx.arc(n.x, n.y, n.r, 0, Math.PI * 2);
            ctx.fillStyle = 'rgba(100, 255, 180, 0.3)';
            ctx.fill();

            for (let j = i + 1; j < nodes.length; j++) {
                const n2 = nodes[j];
                const dx = n.x - n2.x;
                const dy = n.y - n2.y;
                const dist = Math.sqrt(dx*dx + dy*dy);
                if (dist < maxDistance) {
                    ctx.beginPath();
                    ctx.moveTo(n.x, n.y);
                    ctx.lineTo(n2.x, n2.y);
                    ctx.strokeStyle = `rgba(100, 255, 180, ${(1 - dist/maxDistance) * 0.15})`;
                    ctx.stroke();
                }
            }
        });
        requestAnimationFrame(animate);
    }

    window.addEventListener('resize', () => {
        resize();
        initNodes();
    });
    resize();
    initNodes();
    animate();
}

function initPlayground() {
    const svg = d3.select("#playground-canvas");
    const container = document.querySelector('.pg-preview');
    let width = container.clientWidth;
    let height = container.clientHeight;

    const nodes = d3.range(30).map(i => ({ id: i }));
    const links = d3.range(29).map(i => ({ source: i, target: i + 1 }));
    for(let i=0; i<15; i++) {
        links.push({ 
            source: Math.floor(Math.random()*30), 
            target: Math.floor(Math.random()*30) 
        });
    }

    const simulation = d3.forceSimulation(nodes)
        .force("link", d3.forceLink(links).id(d => d.id).distance(50))
        .force("charge", d3.forceManyBody().strength(-200))
        .force("center", d3.forceCenter(width / 2, height / 2));

    const link = svg.append("g")
        .attr("stroke", "rgba(255,255,255,0.1)")
        .selectAll("line")
        .data(links)
        .join("line");

    const node = svg.append("g")
        .selectAll("circle")
        .data(nodes)
        .join("circle")
        .attr("r", 5)
        .attr("fill", "#64ffb4")
        .call(d3.drag()
            .on("start", dragstarted)
            .on("drag", dragged)
            .on("end", dragended));

    simulation.on("tick", () => {
        link.attr("x1", d => d.source.x).attr("y1", d => d.source.y).attr("x2", d => d.target.x).attr("y2", d => d.target.y);
        node.attr("cx", d => d.x).attr("cy", d => d.y);
    });

    function dragstarted(event) {
        if (!event.active) simulation.alphaTarget(0.3).restart();
        event.subject.fx = event.subject.x;
        event.subject.fy = event.subject.y;
    }
    function dragged(event) {
        event.subject.fx = event.x;
        event.subject.fy = event.y;
    }
    function dragended(event) {
        if (!event.active) simulation.alphaTarget(0);
        event.subject.fx = null;
        event.subject.fy = null;
    }

    const repulsionInput = document.getElementById('repulsion');
    const distanceInput = document.getElementById('distance');
    const gravityInput = document.getElementById('gravity');

    repulsionInput.addEventListener('input', (e) => {
        simulation.force("charge").strength(-e.target.value);
        document.getElementById('repulsion-val').innerText = e.target.value;
        simulation.alpha(0.3).restart();
    });
    distanceInput.addEventListener('input', (e) => {
        simulation.force("link").distance(e.target.value);
        document.getElementById('distance-val').innerText = e.target.value;
        simulation.alpha(0.3).restart();
    });
    gravityInput.addEventListener('input', (e) => {
        simulation.force("center", d3.forceCenter(width / 2, height / 2));
        document.getElementById('gravity-val').innerText = e.target.value;
        simulation.alpha(0.3).restart();
    });
}

function initMagneticButtons() {
    const buttons = document.querySelectorAll('.magnetic');
    buttons.forEach(btn => {
        btn.addEventListener('mousemove', (e) => {
            const rect = btn.getBoundingClientRect();
            const x = e.clientX - rect.left - rect.width / 2;
            const y = e.clientY - rect.top - rect.height / 2;
            btn.style.transform = `translate(${x * 0.3}px, ${y * 0.3}px)`;
        });
        btn.addEventListener('mouseleave', () => {
            btn.style.transform = `translate(0, 0)`;
        });
    });
}

function initSpotlight() {
    const cards = document.querySelectorAll('.feature-card');
    cards.forEach(card => {
        card.addEventListener('mousemove', (e) => {
            const rect = card.getBoundingClientRect();
            card.style.setProperty('--x', `${((e.clientX - rect.left) / rect.width) * 100}%`);
            card.style.setProperty('--y', `${((e.clientY - rect.top) / rect.height) * 100}%`);
        });
    });
}

function initScrollReveal() {
    const observer = new IntersectionObserver((entries) => {
        entries.forEach(entry => {
            if (entry.isIntersecting) {
                entry.target.classList.add('reveal');
                observer.unobserve(entry.target);
            }
        });
    }, { threshold: 0.1, rootMargin: '0px 0px -100px 0px' });

    document.querySelectorAll('.section-title, .feature-card, .playground, .marquee-container').forEach(el => {
        el.style.opacity = '0';
        el.style.transform = 'translateY(40px)';
        el.style.transition = 'all 1.2s cubic-bezier(0.16, 1, 0.3, 1)';
        observer.observe(el);
    });
}

function initCustomCursor() {
    const dot = document.querySelector('.cursor-dot');
    const outline = document.querySelector('.cursor-outline');
    if (!dot || !outline) return;

    let mouseX = 0, mouseY = 0;
    let outlineX = 0, outlineY = 0;

    window.addEventListener('mousemove', (e) => {
        mouseX = e.clientX;
        mouseY = e.clientY;
        dot.style.transform = `translate(${mouseX}px, ${mouseY}px) translate(-50%, -50%)`;
    });

    const animateOutline = () => {
        const easing = 0.15;
        outlineX += (mouseX - outlineX) * easing;
        outlineY += (mouseY - outlineY) * easing;
        outline.style.transform = `translate(${outlineX}px, ${outlineY}px) translate(-50%, -50%)`;
        requestAnimationFrame(animateOutline);
    };
    animateOutline();

    const hoverables = document.querySelectorAll('a, button, input, .feature-card, .magnetic');
    hoverables.forEach(el => {
        el.addEventListener('mouseenter', () => {
            outline.style.width = '60px';
            outline.style.height = '60px';
            outline.style.borderWidth = '1px';
            outline.style.background = 'rgba(100, 255, 180, 0.1)';
            outline.style.opacity = '1';
        });
        el.addEventListener('mouseleave', () => {
            outline.style.width = '32px';
            outline.style.height = '32px';
            outline.style.borderWidth = '1.5px';
            outline.style.background = 'transparent';
            outline.style.opacity = '0.4';
        });
    });
}
function initViewportTilt() {
    const window3d = document.querySelector('.viewport-3d-window');
    const container = document.querySelector('.viewport-3d-container');
    if (!window3d || !container) return;

    container.addEventListener('mousemove', (e) => {
        const rect = container.getBoundingClientRect();
        const x = e.clientX - rect.left;
        const y = e.clientY - rect.top;
        
        const xPct = (x / rect.width) - 0.5;
        const yPct = (y / rect.height) - 0.5;
        
        const rotateX = 10 - (yPct * 20); // Tilt based on base 10deg
        const rotateY = -5 + (xPct * 20); // Tilt based on base -5deg
        
        window3d.style.transform = `rotateX(${rotateX}deg) rotateY(${rotateY}deg) rotateZ(2deg) scale(1.02)`;
    });

    container.addEventListener('mouseleave', () => {
        window3d.style.transform = `rotateX(10deg) rotateY(-5deg) rotateZ(2deg) scale(1)`;
    });
}
