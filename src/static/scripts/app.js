// Fragment Review Application

// Application state
let state = {
    selectedFile: null,
    selectedFragmentId: null,
    fragments: []
};

// Initialize the application
function init() {
    loadStateFromLocalStorage();
    fetchAndPopulateFileList();
    setupEventListeners();
    setupResizablePanels();
    setupMenuDropdown();
    console.log('Fragment Review Application initialized');
}

// Load state from localStorage
function loadStateFromLocalStorage() {
    const savedState = localStorage.getItem('fragmentReviewState');
    if (savedState) {
        try {
            const parsed = JSON.parse(savedState);
            state.selectedFile = parsed.selectedFile;
            state.selectedFragmentId = parsed.selectedFragmentId;
        } catch (e) {
            console.error('Error loading state from localStorage:', e);
        }
    }
}

// Save state to localStorage
function saveStateToLocalStorage() {
    const stateToSave = {
        selectedFile: state.selectedFile,
        selectedFragmentId: state.selectedFragmentId
    };
    localStorage.setItem('fragmentReviewState', JSON.stringify(stateToSave));
}

// Fetch and populate file list from API
async function fetchAndPopulateFileList() {
    const fileList = document.getElementById('file-list');
    fileList.innerHTML = '<div class="panel-item">Loading...</div>';
    
    try {
        const response = await fetch('/api/files');
        if (!response.ok) throw new Error('Failed to fetch files');
        
        const files = await response.json();
        fileList.innerHTML = '';
        
        files.forEach(file => {
            const item = document.createElement('div');
            item.className = 'panel-item';
            item.textContent = `${file.filename} (${file.fragment_count})`;
            item.dataset.filename = file.filename;
            item.onclick = () => selectFile(file.filename);
            fileList.appendChild(item);
        });
        
        if (files.length === 0) {
            fileList.innerHTML = '<div class="panel-item">No files found</div>';
        }
        
        // Restore state after loading files
        await restoreStateAfterInit();
    } catch (error) {
        console.error('Error fetching files:', error);
        fileList.innerHTML = '<div class="panel-item has-text-danger">Error loading files</div>';
    }
}

// Select a file and populate fragments
async function selectFile(filename) {
    state.selectedFile = filename;
    saveStateToLocalStorage();
    
    // Update UI: highlight selected file
    document.querySelectorAll('#file-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', item.dataset.filename === filename);
    });
    
    await fetchAndPopulateFragmentList(filename);
    clearFragmentDetails();
}

// Fetch and populate fragment list from API
async function fetchAndPopulateFragmentList(filename) {
    const fragmentList = document.getElementById('fragment-list');
    fragmentList.innerHTML = '<div class="panel-item">Loading...</div>';
    
    try {
        const response = await fetch(`/api/files/${encodeURIComponent(filename)}/fragments`);
        if (!response.ok) throw new Error('Failed to fetch fragments');
        
        const fragments = await response.json();
        state.fragments = fragments;
        
        fragmentList.innerHTML = '';
        
        fragments.forEach(fragment => {
            const item = document.createElement('div');
            item.className = 'panel-item';
            
            const statusColor = getStatusColor(fragment.frag_review);
            item.innerHTML = `
                <span class="tag">${fragment.frag_idx}</span>
                <span class="tag is-${statusColor}">${fragment.frag_type}</span>
                <span class="tag">${fragment.cst_code}</span>
                <span class="tag">${fragment.sc_code}</span>
            `;
            item.dataset.fragmentId = fragment.id;
            item.onclick = () => selectFragment(fragment.id);
            fragmentList.appendChild(item);
        });
        
        if (fragments.length === 0) {
            fragmentList.innerHTML = '<div class="panel-item">No fragments found</div>';
        }
    } catch (error) {
        console.error('Error fetching fragments:', error);
        fragmentList.innerHTML = '<div class="panel-item has-text-danger">Error loading fragments</div>';
    }
}

// Get Bulma color class for review status
function getStatusColor(status) {
    if (!status) return 'light';
    
    const colorMap = {
        'unchecked': 'light',
        'in-progress': 'warning',
        'checked': 'success',
        'needs-review': 'danger'
    };
    return colorMap[status] || 'light';
}

// Select a fragment and show details
async function selectFragment(fragmentId) {
    state.selectedFragmentId = fragmentId;
    saveStateToLocalStorage();
    
    // Update UI: highlight selected fragment
    document.querySelectorAll('#fragment-list .panel-item').forEach(item => {
        item.classList.toggle('is-active', parseInt(item.dataset.fragmentId) === fragmentId);
    });
    
    await fetchAndDisplayFragmentDetails(fragmentId);
}

// Fetch and display fragment details from API
async function fetchAndDisplayFragmentDetails(fragmentId) {
    try {
        const response = await fetch(`/api/fragments/${fragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        
        const detail = await response.json();
        
        // Update metadata fields
        document.getElementById('id').value = detail.id;
        document.getElementById('cst_file').value = detail.cst_file;
        document.getElementById('frag_type').value = detail.frag_type;
        document.getElementById('frag_review').value = detail.frag_review || '';
        document.getElementById('nikaya').value = detail.nikaya;
        document.getElementById('cst_code').value = detail.cst_code || '';
        document.getElementById('cst_vagga').value = detail.cst_vagga || '';
        document.getElementById('cst_sutta').value = detail.cst_sutta || '';
        document.getElementById('cst_paranum').value = detail.cst_paranum || '';
        document.getElementById('sc_code').value = detail.sc_code || '';
        document.getElementById('sc_sutta').value = detail.sc_sutta || '';
        document.getElementById('start_line').value = detail.start_line;
        document.getElementById('start_char').value = detail.start_char;
        document.getElementById('end_line').value = detail.end_line;
        document.getElementById('end_char').value = detail.end_char;
        document.getElementById('group_levels').value = detail.group_levels;
        
        // Update text areas
        const prevTextarea = document.getElementById('prev-content');
        const currentTextarea = document.getElementById('current-content');
        const nextTextarea = document.getElementById('next-content');
        
        prevTextarea.value = detail.prev_fragment ? detail.prev_fragment.content_xml : '';
        currentTextarea.value = detail.content_xml;
        nextTextarea.value = detail.next_fragment ? detail.next_fragment.content_xml : '';
        
        // Scroll previous fragment textarea to bottom
        if (detail.prev_fragment) {
            // Use setTimeout to ensure the textarea has been updated
            setTimeout(() => {
                prevTextarea.scrollTop = prevTextarea.scrollHeight;
            }, 0);
        }
        
        // Enable/disable controls based on position and fragment type
        const hasPrev = detail.prev_fragment !== null;
        const hasNext = detail.next_fragment !== null;
        
        // Delete buttons should only be enabled if the current fragment is Sutta
        // and there's an adjacent Sutta fragment to merge with
        const currentIsSutta = detail.frag_type === 'Sutta';
        const currentIsHeader = detail.frag_type === 'Header';
        const prevIsSutta = detail.prev_fragment && detail.prev_fragment.frag_type === 'Sutta';
        const nextIsSutta = detail.next_fragment && detail.next_fragment.frag_type === 'Sutta';
        const prevIsHeader = detail.prev_fragment && detail.prev_fragment.frag_type === 'Header';
        const nextIsHeader = detail.next_fragment && detail.next_fragment.frag_type === 'Header';
        
        let el;
        el = document.getElementById('delete-prev-btn');
        if (el) {
            // Enable delete-prev button only if current is Sutta and has previous Sutta to merge with
            el.disabled = !(currentIsSutta && prevIsSutta);
        }

        el = document.getElementById('delete-next-btn');
        if (el) {
            // Enable delete-next button only if current is Sutta and has next Sutta to merge with
            el.disabled = !(currentIsSutta && nextIsSutta);
        }

        el = document.getElementById('create-prev-btn');
        if (el) {
            // Create new prev button: disabled if current is the first Header (no prev Header exists)
            el.disabled = currentIsHeader && !prevIsHeader;
        }

        el = document.getElementById('create-next-btn');
        if (el) {
            // Create new next button: disabled if current is the last Header (no next Header exists)
            el.disabled = currentIsHeader && !nextIsHeader;
        }

        // Boundary adjustment buttons enabled if there's a prev/next fragment
        document.querySelectorAll('[id^="prev-"]').forEach(btn => btn.disabled = !hasPrev);
        document.querySelectorAll('[id^="next-"]').forEach(btn => btn.disabled = !hasNext);
        
    } catch (error) {
        console.error('Error fetching fragment details:', error);
        alert('Error loading fragment details');
    }
}

// Clear fragment details
function clearFragmentDetails() {
    document.getElementById('id').value = '';
    document.getElementById('cst_file').value = '';
    document.getElementById('frag_type').value = '';
    document.getElementById('frag_review').value = '';
    document.getElementById('nikaya').value = '';
    document.getElementById('cst_code').value = '';
    document.getElementById('cst_vagga').value = '';
    document.getElementById('cst_sutta').value = '';
    document.getElementById('cst_paranum').value = '';
    document.getElementById('sc_code').value = '';
    document.getElementById('sc_sutta').value = '';
    document.getElementById('start_line').value = '';
    document.getElementById('start_char').value = '';
    document.getElementById('end_line').value = '';
    document.getElementById('end_char').value = '';
    document.getElementById('group_levels').value = '';
    
    document.getElementById('prev-content').value = '';
    document.getElementById('current-content').value = '';
    document.getElementById('next-content').value = '';
}

// Update fragment metadata (auto-save on blur)
async function updateFragmentMetadata() {
    if (!state.selectedFragmentId) return;
    
    const metadata = {
        frag_review: document.getElementById('frag_review').value || null,
        cst_code: document.getElementById('cst_code').value || null,
        sc_code: document.getElementById('sc_code').value || null,
        cst_vagga: document.getElementById('cst_vagga').value || null,
        cst_sutta: document.getElementById('cst_sutta').value || null,
        cst_paranum: document.getElementById('cst_paranum').value || null,
        sc_sutta: document.getElementById('sc_sutta').value || null,
    };
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}`, {
            method: 'PATCH',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(metadata)
        });
        
        if (!response.ok) throw new Error('Failed to update metadata');
        
        // Refresh fragment list to show updated review status
        await fetchAndPopulateFragmentList(state.selectedFile);
        
        console.log('Metadata updated successfully');
    } catch (error) {
        console.error('Error updating metadata:', error);
        alert('Failed to save metadata changes');
    }
}

// Adjust fragment boundary
async function adjustBoundary(action, direction) {
    if (!state.selectedFragmentId) return;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}/adjust-boundary`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ action, direction })
        });
        
        if (!response.ok) throw new Error('Failed to adjust boundary');
        
        const result = await response.json();
        
        // Refresh the current fragment view
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);
        
        console.log('Boundary adjusted:', result.message);
    } catch (error) {
        console.error('Error adjusting boundary:', error);
        alert('Failed to adjust boundary');
    }
}

// Delete adjacent fragment and merge with current fragment
async function deleteFragment(direction) {
    if (!state.selectedFragmentId) return;
    
    // Get current fragment details
    const detail = await getCurrentFragmentDetail();
    if (!detail) return;
    
    // Determine which fragment to delete
    let fragmentIdToDelete;
    if (direction === 'prev') {
        // Delete the PREVIOUS fragment, merge into current
        if (!detail.prev_fragment) return;
        fragmentIdToDelete = detail.prev_fragment.id;
    } else {
        // Delete the NEXT fragment, merge into current
        if (!detail.next_fragment) return;
        fragmentIdToDelete = detail.next_fragment.id;
    }
    
    try {
        // Delete the adjacent fragment (it will be merged into current or next remaining fragment)
        const response = await fetch(`/api/fragments/${fragmentIdToDelete}`, {
            method: 'DELETE'
        });
        
        if (!response.ok) throw new Error('Failed to delete fragment');
        
        // Refresh fragment list
        await fetchAndPopulateFragmentList(state.selectedFile);
        
        // Keep the current fragment selected (it now contains the merged content)
        await fetchAndDisplayFragmentDetails(state.selectedFragmentId);
        
        console.log('Fragment deleted and merged successfully');
    } catch (error) {
        console.error('Error deleting fragment:', error);
        alert('Failed to delete and merge fragment');
    }
}

// Get current fragment detail
async function getCurrentFragmentDetail() {
    if (!state.selectedFragmentId) return null;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}`);
        if (!response.ok) throw new Error('Failed to fetch fragment details');
        return await response.json();
    } catch (error) {
        console.error('Error fetching fragment details:', error);
        return null;
    }
}

// Create new fragment before or after current fragment
async function createNewFragment(direction) {
    if (!state.selectedFragmentId) return;
    
    try {
        const response = await fetch(`/api/fragments/${state.selectedFragmentId}/create`, {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ direction })
        });
        
        if (!response.ok) throw new Error('Failed to create new fragment');
        
        const result = await response.json();
        
        // Refresh fragment list
        await fetchAndPopulateFragmentList(state.selectedFile);
        
        // Select the newly created fragment
        await selectFragment(result.new_fragment_id);
        
        console.log('New fragment created successfully');
    } catch (error) {
        console.error('Error creating new fragment:', error);
        alert('Failed to create new fragment');
    }
}

// Setup event listeners for buttons
function setupEventListeners() {
    // Auto-save metadata on blur
    document.getElementById('frag_review').onchange = updateFragmentMetadata;
    document.getElementById('cst_code').onblur = updateFragmentMetadata;
    document.getElementById('sc_code').onblur = updateFragmentMetadata;
    document.getElementById('cst_vagga').onblur = updateFragmentMetadata;
    document.getElementById('cst_sutta').onblur = updateFragmentMetadata;
    document.getElementById('cst_paranum').onblur = updateFragmentMetadata;
    document.getElementById('sc_sutta').onblur = updateFragmentMetadata;
    
    // Boundary adjustment buttons for previous fragment
    document.getElementById('prev-line-up').onclick = () => adjustBoundary('line_up', 'prev');
    document.getElementById('prev-line-down').onclick = () => adjustBoundary('line_down', 'prev');
    document.getElementById('prev-char-left').onclick = () => adjustBoundary('char_left', 'prev');
    document.getElementById('prev-char-right').onclick = () => adjustBoundary('char_right', 'prev');
    
    // Boundary adjustment buttons for next fragment
    document.getElementById('next-line-up').onclick = () => adjustBoundary('line_up', 'next');
    document.getElementById('next-line-down').onclick = () => adjustBoundary('line_down', 'next');
    document.getElementById('next-char-left').onclick = () => adjustBoundary('char_left', 'next');
    document.getElementById('next-char-right').onclick = () => adjustBoundary('char_right', 'next');

    let el;
    // Delete buttons with confirmation
    el = document.getElementById('delete-prev-btn');
    if (el) {
        el.onclick = () => {
            showConfirmModal('Delete the previous fragment and merge its content with the current fragment?', () => {
                deleteFragment('prev');
            });
        };
    }

    el = document.getElementById('delete-next-btn');
    if (el) {
        el.onclick = () => {
            showConfirmModal('Delete the next fragment and merge its content with the current fragment?', () => {
                deleteFragment('next');
            });
        };
    }
    
    // Create new fragment buttons
    el = document.getElementById('create-prev-btn');
    if (el) {
        el.onclick = () => createNewFragment('prev');
    }
    el = document.getElementById('create-next-btn');
    if (el) {
        el.onclick = () => createNewFragment('next');
    }

    // Modal controls
    document.getElementById('modal-close').onclick = closeModal;
    document.getElementById('modal-cancel').onclick = closeModal;
    
    // Menu buttons
    document.getElementById('menu-settings').onclick = (e) => {
        e.preventDefault();
        closeMenuDropdown();
        openSettingsModal();
    };
    
    document.getElementById('menu-regenerate').onclick = (e) => {
        e.preventDefault();
        closeMenuDropdown();
        openRegenerateModal();
    };
    
    // Settings modal controls
    document.getElementById('settings-modal-close').onclick = closeSettingsModal;
    document.getElementById('settings-cancel').onclick = closeSettingsModal;
    document.getElementById('settings-save').onclick = saveSettings;
    
    // Regenerate modal controls
    document.getElementById('regenerate-modal-close').onclick = closeRegenerateModal;
    document.getElementById('regenerate-close').onclick = closeRegenerateModal;
    document.getElementById('regenerate-with-reference').onclick = () => startRegeneration(true);
    document.getElementById('regenerate-new-replace').onclick = () => startRegeneration(false);
    document.getElementById('copy-output-btn').onclick = copyOutputToClipboard;
}

// Show confirmation modal
function showConfirmModal(message, onConfirm) {
    document.getElementById('modal-message').textContent = message;
    document.getElementById('modal-confirm').onclick = () => {
        onConfirm();
        closeModal();
    };
    document.getElementById('confirm-modal').classList.add('is-active');
}

// Close modal
function closeModal() {
    document.getElementById('confirm-modal').classList.remove('is-active');
}

// Setup resizable panels with draggable separator
function setupResizablePanels() {
    const separator = document.getElementById('separator');
    const leftPanel = document.getElementById('left-panel');
    const rightPanel = document.getElementById('right-panel');
    const container = document.getElementById('main-container');
    
    let isResizing = false;
    
    separator.addEventListener('mousedown', (e) => {
        isResizing = true;
        document.body.style.cursor = 'col-resize';
        document.body.style.userSelect = 'none';
        e.preventDefault();
    });
    
    document.addEventListener('mousemove', (e) => {
        if (!isResizing) return;
        
        const containerRect = container.getBoundingClientRect();
        const newLeftWidth = e.clientX - containerRect.left;
        
        // Enforce min/max constraints
        if (newLeftWidth >= 250 && newLeftWidth <= 600) {
            leftPanel.style.flex = `0 0 ${newLeftWidth}px`;
        }
    });
    
    document.addEventListener('mouseup', () => {
        if (isResizing) {
            isResizing = false;
            document.body.style.cursor = '';
            document.body.style.userSelect = '';
        }
    });
}

// Setup menu dropdown toggle
function setupMenuDropdown() {
    const dropdown = document.getElementById('menu-dropdown');
    const trigger = dropdown.querySelector('.dropdown-trigger button');
    
    trigger.addEventListener('click', (e) => {
        e.stopPropagation();
        dropdown.classList.toggle('is-active');
    });
    
    // Close dropdown when clicking outside
    document.addEventListener('click', (e) => {
        if (!dropdown.contains(e.target)) {
            dropdown.classList.remove('is-active');
        }
    });
}

// Close menu dropdown
function closeMenuDropdown() {
    document.getElementById('menu-dropdown').classList.remove('is-active');
}

// Open Settings modal
async function openSettingsModal() {
    try {
        const response = await fetch('/api/settings');
        if (response.ok) {
            const settings = await response.json();
            document.getElementById('settings-db-path').value = settings.db_path || '';
            document.getElementById('settings-port').value = settings.port || 8000;
            document.getElementById('settings-xml-dir').value = settings.xml_dir || '';
            document.getElementById('settings-xml-filenames').value = (settings.xml_filenames || []).join('\n');
            document.getElementById('settings-parser-path').value = settings.xml_parser_binary_path || 'target/debug/tipitaka_xml_parser';
            document.getElementById('settings-new-db-path').value = settings.new_fragments_db_path || '';
            document.getElementById('settings-ref-db-path').value = settings.reference_fragments_db_path || '';
            document.getElementById('settings-new-tsv-path').value = settings.new_fragments_tsv_path || '';
            document.getElementById('settings-ref-tsv-path').value = settings.reference_fragments_tsv_path || '';
        } else {
            console.error('Failed to load settings');
        }
    } catch (error) {
        console.error('Error loading settings:', error);
    }
    
    document.getElementById('settings-modal').classList.add('is-active');
}

// Close Settings modal
function closeSettingsModal() {
    document.getElementById('settings-modal').classList.remove('is-active');
}

// Save settings
async function saveSettings() {
    const settings = {
        db_path: document.getElementById('settings-db-path').value,
        port: parseInt(document.getElementById('settings-port').value) || 8000,
        xml_dir: document.getElementById('settings-xml-dir').value,
        xml_filenames: document.getElementById('settings-xml-filenames').value
            .split('\n')
            .map(line => line.trim())
            .filter(line => line.length > 0),
        xml_parser_binary_path: document.getElementById('settings-parser-path').value || null,
        new_fragments_db_path: document.getElementById('settings-new-db-path').value || null,
        reference_fragments_db_path: document.getElementById('settings-ref-db-path').value || null,
        new_fragments_tsv_path: document.getElementById('settings-new-tsv-path').value || null,
        reference_fragments_tsv_path: document.getElementById('settings-ref-tsv-path').value || null
    };
    
    try {
        const response = await fetch('/api/settings', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify(settings)
        });
        
        if (response.ok) {
            console.log('Settings saved successfully');
            closeSettingsModal();
            
            // Show success notification
            alert('Settings saved successfully. Some changes may require a server restart.');
        } else {
            console.error('Failed to save settings');
            alert('Failed to save settings');
        }
    } catch (error) {
        console.error('Error saving settings:', error);
        alert('Error saving settings');
    }
}

// Open Regenerate modal
function openRegenerateModal() {
    // Reset modal state
    document.getElementById('regenerate-start-message').style.display = 'block';
    document.getElementById('regenerate-status').style.display = 'none';
    document.getElementById('regenerate-output-container').style.display = 'none';
    document.getElementById('regenerate-output').textContent = '';
    document.getElementById('regenerate-with-reference').disabled = false;
    document.getElementById('regenerate-new-replace').disabled = false;
    
    document.getElementById('regenerate-modal').classList.add('is-active');
}

// Close Regenerate modal
function closeRegenerateModal() {
    document.getElementById('regenerate-modal').classList.remove('is-active');
    
    // If DB was replaced, reload the page now
    if (needsReloadAfterClose) {
        needsReloadAfterClose = false;
        setTimeout(() => {
            window.location.reload();
        }, 100);
    }
}

// Global flag to track if reload is needed
let needsReloadAfterClose = false;

// Start regeneration process
async function startRegeneration(useReferenceDb) {
    // Hide start message, show status
    document.getElementById('regenerate-start-message').style.display = 'none';
    document.getElementById('regenerate-status').style.display = 'block';
    document.getElementById('regenerate-output-container').style.display = 'block';
    document.getElementById('regenerate-with-reference').disabled = true;
    document.getElementById('regenerate-new-replace').disabled = true;
    
    // Reset reload flag
    needsReloadAfterClose = false;
    
    // Update status
    updateRegenerateStatus('Running...', 'is-info');
    
    try {
        const response = await fetch('/api/regenerate', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ use_reference_db: useReferenceDb })
        });
        
        // Always try to parse as JSON since we always return JSON now
        const result = await response.json();
        
        // Display output
        document.getElementById('regenerate-output').textContent = result.output;
        
        // Update status based on success
        if (result.success) {
            if (result.db_replaced) {
                updateRegenerateStatus('Completed successfully! Close modal to reload UI.', 'is-success');
                needsReloadAfterClose = true;
            } else {
                updateRegenerateStatus('Completed successfully!', 'is-success');
            }
        } else {
            // Check if this is a configuration error (output starts with ERROR:)
            if (result.output.startsWith('ERROR:')) {
                updateRegenerateStatus('Configuration Error', 'is-danger');
            } else {
                updateRegenerateStatus('Completed with errors', 'is-warning');
            }
        }
        
    } catch (error) {
        console.error('Regeneration failed:', error);
        updateRegenerateStatus('Failed: ' + error.message, 'is-danger');
        
        // Only append if there's no existing output
        const outputEl = document.getElementById('regenerate-output');
        if (!outputEl.textContent) {
            outputEl.textContent = 'Error: ' + error.message;
        }
    }
}

// Update regeneration status message
function updateRegenerateStatus(message, colorClass) {
    const statusDiv = document.getElementById('regenerate-status');
    const statusText = document.getElementById('regenerate-status-text');
    
    // Remove all color classes
    statusDiv.classList.remove('is-info', 'is-success', 'is-warning', 'is-danger');
    
    // Add new color class
    statusDiv.classList.add(colorClass);
    
    // Update text
    statusText.textContent = message;
}

// Copy output to clipboard
function copyOutputToClipboard() {
    const output = document.getElementById('regenerate-output').textContent;
    
    navigator.clipboard.writeText(output).then(() => {
        // Change button text temporarily to show success
        const btn = document.getElementById('copy-output-btn');
        const originalText = btn.innerHTML;
        btn.innerHTML = '<span>✓ Copied!</span>';
        btn.classList.add('is-success');
        
        setTimeout(() => {
            btn.innerHTML = originalText;
            btn.classList.remove('is-success');
        }, 2000);
    }).catch(err => {
        console.error('Failed to copy:', err);
        alert('Failed to copy to clipboard');
    });
}

// Restore state after loading file list
async function restoreStateAfterInit() {
    if (state.selectedFile) {
        // Try to select the previously selected file
        const fileItem = document.querySelector(`#file-list .panel-item[data-filename="${state.selectedFile}"]`);
        if (fileItem) {
            await selectFile(state.selectedFile);
            
            // Try to select the previously selected fragment
            if (state.selectedFragmentId) {
                const fragmentItem = document.querySelector(`#fragment-list .panel-item[data-fragment-id="${state.selectedFragmentId}"]`);
                if (fragmentItem) {
                    await selectFragment(state.selectedFragmentId);
                }
            }
        }
    }
}

// Initialize when DOM is ready
if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
} else {
    init();
}
