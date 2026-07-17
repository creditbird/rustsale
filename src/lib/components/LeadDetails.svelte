<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import * as crmService from '$lib/services/crmService';
  import type { Lead, Activity } from '$lib/services/crmService';

  let { leadId, onClose } = $props<{ leadId: number; onClose: () => void }>();

  let lead = $state<Lead | null>(null);
  let activities = $state<Activity[]>([]);
  let loading = $state(false);
  let errorMsg = $state<string | null>(null);

  // Form states
  let activityType = $state('Call');
  let activityContent = $state('');
  let logging = $state(false);

  // Fetch details when leadId changes
  $effect(() => {
    if (leadId) {
      loadLeadDetails(leadId);
    }
  });

  async function loadLeadDetails(id: number) {
    loading = true;
    errorMsg = null;
    try {
      const [leadData, activitiesData] = await Promise.all([
        crmService.getLeadDetails(id),
        crmService.getActivities(id)
      ]);
      if (id !== leadId) return;
      lead = leadData;
      // Sort activities: newest first
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    } catch (err) {
      if (id === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (id === leadId) {
        loading = false;
      }
    }
  }

  async function handleLogActivity(e: Event) {
    e.preventDefault();
    if (!activityContent.trim()) return;

    logging = true;
    errorMsg = null;
    const currentLeadId = leadId;
    try {
      const performer = lead?.owner || 'Self';
      await crmService.addActivity(currentLeadId, activityType, activityContent.trim(), performer);
      if (currentLeadId !== leadId) return;
      activityContent = '';
      
      // Refresh activity list
      const activitiesData = await crmService.getActivities(currentLeadId);
      if (currentLeadId !== leadId) return;
      activities = activitiesData.sort(
        (a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime()
      );
    } catch (err) {
      if (currentLeadId === leadId) {
        errorMsg = err instanceof Error ? err.message : String(err);
      }
    } finally {
      if (currentLeadId === leadId) {
        logging = false;
      }
    }
  }

  function getActivityIcon(type: string) {
    switch (type.toLowerCase()) {
      case 'call': return 'call';
      case 'email': return 'email';
      case 'meeting': return 'groups';
      default: return 'event_note';
    }
  }

  function formatDateTime(dateStr: string) {
    if (!dateStr) return '';
    try {
      const date = new Date(dateStr);
      if (isNaN(date.getTime())) return dateStr;
      return date.toLocaleString('en-US', {
        month: 'short',
        day: 'numeric',
        hour: '2-digit',
        minute: '2-digit'
      });
    } catch (e) {
      return dateStr;
    }
  }
</script>

<!-- Backdrop overlay -->
<div 
  class="drawer-backdrop" 
  transition:fade={{ duration: 150 }} 
  onclick={onClose}
  role="presentation"
></div>

<!-- Drawer container -->
<div 
  class="drawer-container" 
  transition:fly={{ x: 400, duration: 250 }}
>
  <header class="drawer-header">
    <h3>Lead Details</h3>
    <button class="close-btn" onclick={onClose} aria-label="Close details">
      <span class="material-icons">close</span>
    </button>
  </header>

  {#if loading && !lead}
    <div class="drawer-state">
      <span class="spinner"></span>
      <p>Loading details...</p>
    </div>
  {:else if errorMsg && !lead}
    <div class="drawer-state error">
      <span class="material-icons">error_outline</span>
      <p>{errorMsg}</p>
    </div>
  {:else if lead}
    <div class="drawer-content">
      {#if errorMsg}
        <div class="error-banner">
          <span class="material-icons">error_outline</span>
          <span>{errorMsg}</span>
        </div>
      {/if}

      <!-- Profile Summary -->
      <section class="profile-summary">
        <div class="avatar">
          {lead.name.split(' ').map((n: string) => n[0]).join('').toUpperCase().substring(0, 2)}
        </div>
        <div class="profile-info">
          <h4>{lead.name}</h4>
          <p class="company">{lead.company}</p>
          <span class="status-badge {lead.status.toLowerCase()}">{lead.status}</span>
        </div>
      </section>

      <!-- Details List -->
      <section class="details-section">
        <h5>Contact Details</h5>
        <div class="detail-row">
          <span class="detail-label">Email</span>
          <span class="detail-value"><a href="mailto:{lead.email}">{lead.email}</a></span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Phone</span>
          <span class="detail-value">{lead.phone || '—'}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Lead Owner</span>
          <span class="detail-value">{lead.owner || '—'}</span>
        </div>
        <div class="detail-row">
          <span class="detail-label">Est. Value</span>
          <span class="detail-value value-text">${(lead.value || 0).toLocaleString()}</span>
        </div>
        {#if lead.about}
          <div class="about-block">
            <span class="detail-label">About</span>
            <p>{lead.about}</p>
          </div>
        {/if}
      </section>

      <!-- Log Activity -->
      <section class="log-activity-section">
        <h5>Log Activity</h5>
        <form onsubmit={handleLogActivity} class="activity-form">
          <div class="form-row">
            <label for="act-type" class="sr-only">Type</label>
            <select id="act-type" bind:value={activityType} class="activity-select">
              <option value="Call">Call</option>
              <option value="Email">Email</option>
              <option value="Meeting">Meeting</option>
            </select>
          </div>
          <div class="form-row">
            <label for="act-content" class="sr-only">Activity notes</label>
            <textarea
              id="act-content"
              bind:value={activityContent}
              placeholder="What did you discuss? Enter notes here..."
              rows="3"
              required
            ></textarea>
          </div>
          <button type="submit" class="submit-btn" disabled={logging || !activityContent.trim()}>
            {#if logging}
              Logging...
            {:else}
              Log Activity
            {/if}
          </button>
        </form>
      </section>

      <!-- Activity History -->
      <section class="activities-section">
        <h5>Activity History</h5>
        {#if activities.length === 0}
          <p class="no-activities">No activities logged yet.</p>
        {:else}
          <div class="timeline">
            {#each activities as activity}
              <div class="timeline-item">
                <div class="timeline-icon">
                  <span class="material-icons">{getActivityIcon(activity.activity_type)}</span>
                </div>
                <div class="timeline-content">
                  <div class="timeline-meta">
                    <span class="activity-type">{activity.activity_type}</span>
                    <span class="activity-time">{formatDateTime(activity.created_at)}</span>
                  </div>
                  <p class="activity-text">{activity.content}</p>
                  <span class="activity-performer">Logged by {activity.performed_by}</span>
                </div>
              </div>
            {/each}
          </div>
        {/if}
      </section>
    </div>
  {/if}
</div>

<style>
  .drawer-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(15, 23, 42, 0.15);
    backdrop-filter: blur(2px);
    z-index: 1000;
  }

  .drawer-container {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: 400px;
    max-width: 100%;
    background: var(--color-surface-container-lowest);
    border-left: 1px solid var(--color-outline-variant);
    box-shadow: 0 8px 16px rgba(15, 23, 42, 0.08);
    z-index: 1001;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .drawer-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    padding: var(--spacing-md);
    border-bottom: 1px solid var(--color-outline-variant);
  }

  .drawer-header h3 {
    font-size: 16px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .close-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-on-surface-variant);
    border-radius: var(--rounded-full);
    padding: var(--spacing-xs);
    transition: var(--transition-fast);
  }

  .close-btn:hover {
    background: var(--color-surface-container-low);
    color: var(--color-on-surface);
  }

  .drawer-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    flex: 1;
    padding: var(--spacing-lg);
    gap: var(--spacing-md);
    color: var(--color-on-surface-variant);
  }

  .drawer-state.error {
    color: var(--color-danger);
  }

  .drawer-state .material-icons {
    font-size: 32px;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--color-outline-variant);
    border-top-color: var(--color-success);
    border-radius: var(--rounded-full);
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .drawer-content {
    flex: 1;
    overflow-y: auto;
    padding: var(--spacing-md);
    display: flex;
    flex-direction: column;
    gap: var(--spacing-lg);
  }

  .profile-summary {
    display: flex;
    align-items: center;
    gap: var(--spacing-md);
    padding: var(--spacing-md);
    background: var(--color-surface-container-low);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
  }

  .avatar {
    width: 48px;
    height: 48px;
    border-radius: var(--rounded-full);
    background: var(--color-surface-dim);
    color: var(--color-on-surface);
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
    font-size: 16px;
    border: 1px solid var(--color-outline-variant);
  }

  .profile-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .profile-info h4 {
    font-size: 15px;
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .profile-info .company {
    font-size: 12px;
    color: var(--color-on-surface-variant);
  }

  .status-badge {
    align-self: flex-start;
    padding: 2px 8px;
    border-radius: var(--rounded-full);
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    margin-top: 4px;
  }

  .status-badge.new {
    background: #e0f2fe;
    color: #0369a1;
  }

  .status-badge.contacted {
    background: #fef3c7;
    color: #b45309;
  }

  .status-badge.qualified {
    background: rgba(16, 185, 129, 0.1);
    color: var(--color-success);
  }

  .status-badge.lost {
    background: rgba(244, 63, 94, 0.1);
    color: var(--color-danger);
  }

  .status-badge.converted {
    background: #e0e7ff;
    color: #4338ca;
  }

  .details-section h5, 
  .log-activity-section h5, 
  .activities-section h5 {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--color-on-surface-variant);
    font-weight: 700;
    margin-bottom: var(--spacing-sm);
  }

  .detail-row {
    display: flex;
    justify-content: space-between;
    padding-block: var(--spacing-xs);
    border-bottom: 1px dashed var(--color-outline-variant);
    font-size: 13px;
  }

  .detail-label {
    color: var(--color-on-surface-variant);
  }

  .detail-value {
    color: var(--color-on-surface);
    font-weight: 500;
  }

  .detail-value a {
    color: var(--color-success);
  }

  .detail-value a:hover {
    text-decoration: underline;
  }

  .value-text {
    color: var(--color-success);
    font-weight: 600;
  }

  .about-block {
    margin-top: var(--spacing-sm);
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 13px;
  }

  .about-block p {
    background: var(--color-surface-container-low);
    padding: var(--spacing-sm);
    border-radius: var(--rounded-md);
    color: var(--color-on-surface);
    border: 1px solid var(--color-outline-variant);
    line-height: 1.4;
  }

  .activity-form {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-sm);
  }

  .activity-select {
    padding: var(--spacing-xs) var(--spacing-sm);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: var(--font-family);
    font-size: 13px;
    width: 100%;
    outline: none;
    transition: var(--transition-fast);
  }

  .activity-select:focus {
    border-color: var(--color-outline);
  }

  textarea {
    width: 100%;
    padding: var(--spacing-sm);
    border-radius: var(--rounded-md);
    border: 1px solid var(--color-outline-variant);
    background: var(--color-surface-container-lowest);
    color: var(--color-on-surface);
    font-family: var(--font-family);
    font-size: 13px;
    resize: vertical;
    outline: none;
    transition: var(--transition-fast);
  }

  textarea:focus {
    border-color: var(--color-outline);
  }

  .error-banner {
    display: flex;
    align-items: center;
    gap: var(--spacing-sm);
    padding: var(--spacing-md);
    background-color: rgba(244, 63, 94, 0.08);
    border: 1px solid var(--color-danger);
    border-radius: var(--rounded-md);
    color: var(--color-danger);
    font-size: 13px;
    margin-bottom: var(--spacing-md);
  }

  .submit-btn {
    background: var(--color-primary);
    color: var(--color-on-primary);
    padding: var(--spacing-sm) var(--spacing-md);
    border-radius: var(--rounded-md);
    font-weight: 600;
    font-size: 13px;
    text-align: center;
    transition: var(--transition-fast);
  }

  .submit-btn:hover:not(:disabled) {
    background: var(--color-primary-container);
    color: var(--color-on-primary-container);
  }

  .submit-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .no-activities {
    font-size: 12px;
    color: var(--color-on-surface-variant);
    font-style: italic;
    text-align: center;
    padding-block: var(--spacing-md);
  }

  .timeline {
    display: flex;
    flex-direction: column;
    gap: var(--spacing-md);
    position: relative;
    padding-left: 12px;
  }

  .timeline::before {
    content: '';
    position: absolute;
    top: 8px;
    bottom: 8px;
    left: 20px;
    width: 2px;
    background: var(--color-outline-variant);
  }

  .timeline-item {
    display: flex;
    gap: var(--spacing-md);
    position: relative;
  }

  .timeline-icon {
    width: 24px;
    height: 24px;
    border-radius: var(--rounded-full);
    background: var(--color-surface-container-lowest);
    border: 2px solid var(--color-outline-variant);
    color: var(--color-on-surface-variant);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1;
  }

  .timeline-icon .material-icons {
    font-size: 14px;
  }

  .timeline-content {
    flex: 1;
    background: var(--color-surface-container-lowest);
    border: 1px solid var(--color-outline-variant);
    border-radius: var(--rounded-md);
    padding: var(--spacing-sm);
    display: flex;
    flex-direction: column;
    gap: 4px;
    box-shadow: var(--shadow-soft);
  }

  .timeline-meta {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 11px;
  }

  .activity-type {
    font-weight: 700;
    color: var(--color-on-surface);
  }

  .activity-time {
    color: var(--color-on-surface-variant);
  }

  .activity-text {
    font-size: 13px;
    color: var(--color-on-surface);
    line-height: 1.4;
    word-break: break-word;
  }

  .activity-performer {
    font-size: 10px;
    color: var(--color-on-surface-variant);
    align-self: flex-end;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }
</style>
