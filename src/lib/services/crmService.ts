import { invoke } from '@tauri-apps/api/core';
import { subscribeToGoogleMapsJobEvents, type GoogleMapsJobEvent } from './googleMapsScraperService';

export interface Lead {
  id?: number;
  name: string;
  company: string;
  status: string;
  value: number;
  last_activity: string;
  owner: string;
  email: string;
  phone: string;
  about?: string | null;
  address?: string | null;
  has_zalo?: number | null; // null = unchecked, 0 = no, 1 = yes
  zalo_reached?: number;     // 0 = no, 1 = yes
}

export interface Deal {
  id?: number;
  lead_id: number;
  title: string;
  value: number;
  stage: string;
  created_at: string;
}

export interface Activity {
  id?: number;
  lead_id: number;
  activity_type: string;
  content: string;
  created_at: string;
  performed_by: string;
}

export async function getLeads(): Promise<Lead[]> {
  return invoke<Lead[]>('get_leads');
}

export async function getLeadDetails(id: number): Promise<Lead> {
  return invoke<Lead>('get_lead_details', { id });
}

type LeadsChangeListener = () => void;
const leadsChangeListeners = new Set<LeadsChangeListener>();

export function onLeadsChanged(listener: LeadsChangeListener): () => void {
  leadsChangeListeners.add(listener);
  return () => {
    leadsChangeListeners.delete(listener);
  };
}

export function notifyLeadsChanged(): void {
  leadsChangeListeners.forEach((listener) => {
    try {
      listener();
    } catch (err) {
      console.error('Error in leadsChangeListener', err);
    }
  });
}

// Scraper event bridge delta tracking (prevents request storms)
const lastLeadsCreatedByJob = new Map<string, number>();
const BRIDGE_RETRY_BASE_DELAY_MS = 100;
const BRIDGE_RETRY_MAX_DELAY_MS = 5_000;
let bridgeRefCount = 0;
let bridgeUnlisten: (() => void) | null = null;
let bridgePromise: Promise<() => void> | null = null;
let bridgeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let bridgeRetryAttempt = 0;

function clearBridgeRetry(): void {
  if (bridgeRetryTimer) {
    clearTimeout(bridgeRetryTimer);
    bridgeRetryTimer = null;
  }
  bridgeRetryAttempt = 0;
}

function scheduleBridgeRetry(): void {
  if (bridgeRefCount <= 0 || bridgeRetryTimer || bridgeUnlisten || bridgePromise) return;

  const delay = Math.min(
    BRIDGE_RETRY_BASE_DELAY_MS * 2 ** bridgeRetryAttempt,
    BRIDGE_RETRY_MAX_DELAY_MS
  );
  bridgeRetryAttempt += 1;
  bridgeRetryTimer = setTimeout(() => {
    bridgeRetryTimer = null;
    ensureScraperEventBridgeConnected();
  }, delay);
}

export function handleScraperJobEvent(event: GoogleMapsJobEvent): boolean {
  const previous = lastLeadsCreatedByJob.get(event.job_id) ?? 0;
  const current = event.leads_created;
  const isTerminal = ['completed', 'failed', 'cancelled', 'partial'].includes(event.status);

  let shouldNotify = false;

  if (current > previous) {
    lastLeadsCreatedByJob.set(event.job_id, current);
    shouldNotify = true;
  }

  if (isTerminal) {
    shouldNotify = true;
  }

  if (shouldNotify) {
    notifyLeadsChanged();
    return true;
  }

  return false;
}

export function resetScraperJobEventBridgeState(): void {
  lastLeadsCreatedByJob.clear();
  clearBridgeRetry();
  if (bridgeUnlisten) {
    bridgeUnlisten();
    bridgeUnlisten = null;
  }
  bridgeRefCount = 0;
  bridgePromise = null;
}

export function getBridgeRefCount(): number {
  return bridgeRefCount;
}

export function getBridgePromise(): Promise<() => void> | null {
  return bridgePromise;
}

export function ensureScraperEventBridgeConnected(): Promise<() => void> | null {
  if (bridgeRefCount > 0 && !bridgeUnlisten && !bridgePromise) {
    if (bridgeRetryTimer) {
      clearTimeout(bridgeRetryTimer);
      bridgeRetryTimer = null;
    }
    bridgePromise = subscribeToGoogleMapsJobEvents((event) => {
      handleScraperJobEvent(event);
    }).then((cleanup) => {
      bridgeUnlisten = cleanup;
      bridgeRetryAttempt = 0;
      if (bridgeRefCount <= 0 && bridgeUnlisten) {
        bridgeUnlisten();
        bridgeUnlisten = null;
        bridgePromise = null;
      }
      return cleanup;
    }).catch((err) => {
      bridgePromise = null;
      bridgeUnlisten = null;
      console.error('Failed to init scraper event bridge', err);
      scheduleBridgeRetry();
      return () => {};
    });
  }
  return bridgePromise;
}

export function initScraperEventBridge(): () => void {
  bridgeRefCount += 1;
  ensureScraperEventBridgeConnected();

  let cleaned = false;
  return () => {
    if (cleaned) return;
    cleaned = true;
    bridgeRefCount = Math.max(0, bridgeRefCount - 1);

    if (bridgeRefCount === 0) {
      clearBridgeRetry();
      if (bridgeUnlisten) {
        bridgeUnlisten();
        bridgeUnlisten = null;
        bridgePromise = null;
      }
    }
  };
}

export async function addLead(
  name: string,
  company: string,
  status: string,
  value: number,
  owner: string,
  email: string,
  phone: string,
  about?: string | null,
  address?: string | null
): Promise<number> {
  const result = await invoke<number>('add_lead', {
    name,
    company,
    status,
    value,
    owner,
    email,
    phone,
    about: about ?? null,
    address: address ?? null
  });
  notifyLeadsChanged();
  return result;
}

export async function updateLeadStatus(id: number, status: string): Promise<void> {
  await invoke<void>('update_lead_status', { id, status });
  notifyLeadsChanged();
}

export async function getDeals(): Promise<Deal[]> {
  return invoke<Deal[]>('get_deals');
}

export async function createDeal(
  leadId: number,
  title: string,
  value: number,
  stage: string
): Promise<Deal> {
  const result = await invoke<Deal>('create_deal', {
    leadId,
    title,
    value,
    stage
  });
  notifyLeadsChanged();
  return result;
}

export async function updateDealStage(id: number, stage: string): Promise<void> {
  await invoke<void>('update_deal_stage', { id, stage });
  notifyLeadsChanged();
}

export async function deleteDeal(id: number): Promise<void> {
  await invoke<void>('delete_deal', { id });
  notifyLeadsChanged();
}

export async function getActivities(leadId: number): Promise<Activity[]> {
  return invoke<Activity[]>('get_activities', { leadId });
}

export async function getRecentActivities(limit: number = 10): Promise<Activity[]> {
  return invoke<Activity[]>('get_recent_activities', { limit });
}

export async function addActivity(
  leadId: number,
  activityType: string,
  content: string,
  performedBy: string
): Promise<void> {
  await invoke<void>('add_activity', {
    leadId,
    activityType,
    content,
    performedBy
  });
  notifyLeadsChanged();
}

export async function updateLead(
  id: number,
  name: string,
  company: string,
  status: string,
  value: number,
  owner: string,
  email: string,
  phone: string,
  about?: string | null,
  address?: string | null
): Promise<void> {
  await invoke<void>('update_lead', {
    id,
    name,
    company,
    status,
    value,
    owner,
    email,
    phone,
    about: about ?? null,
    address: address ?? null
  });
  notifyLeadsChanged();
}

export async function deleteLead(id: number): Promise<void> {
  await invoke<void>('delete_lead', { id });
  notifyLeadsChanged();
}

export interface Category {
  id?: number;
  name: string;
}

export async function getCategories(): Promise<Category[]> {
  return invoke<Category[]>('get_categories');
}

export async function createCategory(name: string): Promise<number> {
  const result = await invoke<number>('create_category', { name });
  notifyLeadsChanged();
  return result;
}

export async function deleteCategory(id: number): Promise<void> {
  await invoke<void>('delete_category', { id });
  notifyLeadsChanged();
}

export async function addLeadsToCategory(leadIds: number[], categoryId: number): Promise<void> {
  await invoke<void>('add_leads_to_category', { leadIds, categoryId });
  notifyLeadsChanged();
}

export async function removeLeadsFromCategory(leadIds: number[], categoryId: number): Promise<void> {
  await invoke<void>('remove_leads_from_category', { leadIds, categoryId });
  notifyLeadsChanged();
}

export async function getLeadsByCategory(categoryId: number): Promise<Lead[]> {
  return invoke<Lead[]>('get_leads_by_category', { categoryId });
}

export async function getCampaignLeads(
  categoryId: number,
  segmentation: string,
  filterTemplateId?: number | null,
  channel?: string | null
): Promise<Lead[]> {
  return invoke<Lead[]>('get_campaign_leads', {
    categoryId,
    segmentation,
    filterTemplateId: filterTemplateId ?? null,
    channel: channel ?? null
  });
}
