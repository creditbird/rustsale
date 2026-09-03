import { invoke } from '@tauri-apps/api/core';

export interface TemplateFolderRecord {
  id: number;
  name: string;
  color?: string | null;
  created_at: string;
  updated_at: string;
}

export interface TemplateRecord {
  id: number;
  folder_id?: number | null;
  name: string;
  channel: string;
  stage?: string | null;
  subject?: string | null;
  content: string;
  media_url?: string | null;
  ai_prompt?: string | null;
  tags_json?: string | null;
  usage_count: number;
  last_used_at?: string | null;
  created_at: string;
  updated_at: string;
}

export interface CreateTemplateInput {
  folder_id?: number | null;
  name: string;
  channel: string;
  stage?: string | null;
  subject?: string | null;
  content: string;
  media_url?: string | null;
  ai_prompt?: string | null;
  tags_json?: string | null;
}

export interface UpdateTemplateInput {
  id: number;
  folder_id?: number | null;
  name: string;
  channel: string;
  stage?: string | null;
  subject?: string | null;
  content: string;
  media_url?: string | null;
  ai_prompt?: string | null;
  tags_json?: string | null;
}

export interface TemplateFilter {
  channel?: string | null;
  folder_id?: number | null;
  stage?: string | null;
  search_query?: string | null;
  tag?: string | null;
}

export async function getTemplates(filter?: TemplateFilter): Promise<TemplateRecord[]> {
  return invoke<TemplateRecord[]>('get_templates', { filter: filter || null });
}

export async function getTemplateById(id: number): Promise<TemplateRecord | null> {
  return invoke<TemplateRecord | null>('get_template_by_id', { id });
}

export async function createTemplate(input: CreateTemplateInput): Promise<number> {
  return invoke<number>('create_template', { input });
}

export async function updateTemplate(input: UpdateTemplateInput): Promise<void> {
  return invoke<void>('update_template', { input });
}

export async function deleteTemplate(id: number): Promise<void> {
  return invoke<void>('delete_template', { id });
}

export async function getTemplateFolders(): Promise<TemplateFolderRecord[]> {
  return invoke<TemplateFolderRecord[]>('get_template_folders');
}

export async function createTemplateFolder(name: string, color?: string): Promise<number> {
  return invoke<number>('create_template_folder', { name, color: color || null });
}

export async function deleteTemplateFolder(id: number): Promise<void> {
  return invoke<void>('delete_template_folder', { id });
}

export async function incrementTemplateUsage(id: number): Promise<void> {
  return invoke<void>('increment_template_usage', { id });
}

/**
 * Helper: Parse JSON string of tags safely into a string array.
 */
export function parseTags(tagsJson?: string | null): string[] {
  if (!tagsJson) return [];
  try {
    const parsed = JSON.parse(tagsJson);
    if (Array.isArray(parsed)) {
      return parsed.map((t) => String(t).trim()).filter(Boolean);
    }
  } catch {
    // Fallback: comma separated string
    return tagsJson
      .split(',')
      .map((t) => t.trim())
      .filter(Boolean);
  }
  return [];
}

/**
 * Helper: Convert tag string or array into JSON string.
 */
export function stringifyTags(tags: string[] | string): string {
  if (Array.isArray(tags)) {
    return JSON.stringify(tags.map((t) => t.trim()).filter(Boolean));
  }
  const list = tags
    .split(',')
    .map((t) => t.trim())
    .filter(Boolean);
  return JSON.stringify(list);
}

/**
 * Helper: Resolve Spintax syntax `{A|B|C}` and replace placeholder variables.
 */
export function renderSpintaxPreview(
  text: string,
  sampleData: Record<string, string> = {
    name: 'Nguyễn Văn A',
    company: 'Công ty Á Châu',
    phone: '0987654321',
    email: 'nguyenvana@gmail.com',
    value: '15,000,000 VND'
  }
): string {
  if (!text) return '';
  let result = text;

  for (const [key, val] of Object.entries(sampleData)) {
    const regex = new RegExp(`\\{\\{${key}\\}\\}`, 'gi');
    result = result.replace(regex, val);
  }

  const spintaxRegex = /\{([^{}]+)\}/g;
  let safetyCounter = 0;
  while (spintaxRegex.test(result) && safetyCounter < 10) {
    result = result.replace(spintaxRegex, (_, choices) => {
      const options = choices.split('|');
      return options[Math.floor(Math.random() * options.length)] || '';
    });
    safetyCounter++;
  }

  return result;
}
