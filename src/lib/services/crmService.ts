import { invoke } from '@tauri-apps/api/core';

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
  return invoke<number>('add_lead', {
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
}

export async function updateLeadStatus(id: number, status: string): Promise<void> {
  return invoke<void>('update_lead_status', { id, status });
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
  return invoke<Deal>('create_deal', {
    leadId,
    title,
    value,
    stage
  });
}

export async function updateDealStage(id: number, stage: string): Promise<void> {
  return invoke<void>('update_deal_stage', { id, stage });
}

export async function deleteDeal(id: number): Promise<void> {
  return invoke<void>('delete_deal', { id });
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
  return invoke<void>('add_activity', {
    leadId,
    activityType,
    content,
    performedBy
  });
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
  return invoke<void>('update_lead', {
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
}

export async function deleteLead(id: number): Promise<void> {
  return invoke<void>('delete_lead', { id });
}

export interface Category {
  id?: number;
  name: string;
}

export async function getCategories(): Promise<Category[]> {
  return invoke<Category[]>('get_categories');
}

export async function createCategory(name: string): Promise<number> {
  return invoke<number>('create_category', { name });
}

export async function deleteCategory(id: number): Promise<void> {
  return invoke<void>('delete_category', { id });
}

export async function addLeadsToCategory(leadIds: number[], categoryId: number): Promise<void> {
  return invoke<void>('add_leads_to_category', { leadIds, categoryId });
}

export async function removeLeadsFromCategory(leadIds: number[], categoryId: number): Promise<void> {
  return invoke<void>('remove_leads_from_category', { leadIds, categoryId });
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

