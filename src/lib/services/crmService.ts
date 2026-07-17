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
  about?: string | null
): Promise<number> {
  return invoke<number>('add_lead', {
    name,
    company,
    status,
    value,
    owner,
    email,
    phone,
    about
  });
}

export async function updateLeadStatus(id: number, status: string): Promise<void> {
  return invoke<void>('update_lead_status', { id, status });
}

export async function getDeals(): Promise<Deal[]> {
  return invoke<Deal[]>('get_deals');
}

export async function updateDealStage(id: number, stage: string): Promise<void> {
  return invoke<void>('update_deal_stage', { id, stage });
}

export async function getActivities(leadId: number): Promise<Activity[]> {
  return invoke<Activity[]>('get_activities', { leadId });
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
  about?: string | null
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
    about
  });
}

export async function deleteLead(id: number): Promise<void> {
  return invoke<void>('delete_lead', { id });
}
