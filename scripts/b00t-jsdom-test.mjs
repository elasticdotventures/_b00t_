// b00t-jsdom-test — Execute served HTML in jsdom, check for runtime errors
import { JSDOM } from 'jsdom';

const chunks = [];
process.stdin.on('data', c => chunks.push(c));
process.stdin.on('end', () => {
    const html = Buffer.concat(chunks).toString();
    const dom = new JSDOM(html, { 
        url: 'http://localhost:31337/',
        runScripts: 'dangerously',
        pretendToBeVisual: true 
    });
    
    const { window } = dom;
    const errors = [];
    
    // Collect errors during script execution (ignore CDN fetch failures)
    window.addEventListener('error', e => {
        const msg = e.message || '';
        if (!msg.includes('cytoscape') && !msg.includes('mermaid') 
            && !msg.includes('cdn') && !msg.includes('NODE_PATH')) {
            errors.push(`Runtime: ${msg}`);
        }
    });

    // Check key functions exist
    for (const fn of ['toggleSection', 'beat', 'renderMermaid', 'onVizSelect', 'loadKnowledgeGraph']) {
        if (typeof window[fn] !== 'function') {
            errors.push(`Missing function: ${fn}`);
        }
    }

    if (errors.length) {
        errors.forEach(e => console.error('❌', e));
        process.exit(1);
    }
    console.log('✅ JS executed, all functions defined');
    process.exit(0);
});
