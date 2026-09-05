<script lang="ts">
  import type { CampaignStep } from '$lib/services/unifiedCampaignService';
  import {
    Plus,
    Trash2,
    ArrowUp,
    ArrowDown,
    Clock,
    Filter,
    Bot,
    Sparkles,
    MessageSquare,
    MessageCircleMore,
    Mail,
    Share2,
    Eye,
    HelpCircle,
    ChevronDown,
    ChevronUp,
    FolderOpen,
    Save,
    BookOpen
  } from 'lucide-svelte';
  import { Button } from '$lib/components/ui/button';
  import { Badge } from '$lib/components/ui/badge';
  import TemplatePickerModal from '$lib/components/template/TemplatePickerModal.svelte';
  import SaveTemplateModal from '$lib/components/template/SaveTemplateModal.svelte';
  import { incrementTemplateUsage, type TemplateRecord } from '$lib/services/templateService';

  let {
    steps = $bindable<CampaignStep[]>([]),
    readonly = false
  }: {
    steps: CampaignStep[];
    readonly?: boolean;
  } = $props();

  let previewModalOpen = $state(false);
  let activePreviewStepIndex = $state<number | null>(null);
  let previewSampleOutput = $state('');

  let pickerModalOpen = $state(false);
  let activePickerStepIndex = $state<number | null>(null);
  let pickerChannel = $state('zalo');

  let saveModalOpen = $state(false);
  let activeSaveStepIndex = $state<number | null>(null);
  let saveInitialContent = $state('');
  let saveInitialChannel = $state('zalo');

  function openPickerForStep(index: number) {
    activePickerStepIndex = index;
    pickerChannel = steps[index]?.channel || 'zalo';
    pickerModalOpen = true;
  }

  function handleTemplateSelected(template: TemplateRecord) {
    if (activePickerStepIndex !== null && steps[activePickerStepIndex]) {
      steps[activePickerStepIndex].template_content = template.content;
      steps[activePickerStepIndex].template_id = template.id;
      if (template.ai_prompt && !steps[activePickerStepIndex].ai_prompt_override) {
        steps[activePickerStepIndex].ai_prompt_override = template.ai_prompt;
      }
      incrementTemplateUsage(template.id).catch((err) => {
        console.warn('Failed to increment template usage:', err);
      });
    }
  }

  function openSaveForStep(index: number) {
    activeSaveStepIndex = index;
    saveInitialContent = steps[index]?.template_content || '';
    saveInitialChannel = steps[index]?.channel || 'zalo';
    saveModalOpen = true;
  }

  function handleTemplateSaved(savedId: number) {
    if (activeSaveStepIndex !== null && steps[activeSaveStepIndex]) {
      steps[activeSaveStepIndex].template_id = savedId;
    }
  }

  const CHANNEL_OPTIONS = [
    { id: 'zalo', label: 'Zalo', icon: MessageSquare, color: 'text-blue-600 bg-blue-500/10 border-blue-500/30' },
    { id: 'whatsapp', label: 'WhatsApp', icon: MessageCircleMore, color: 'text-emerald-600 bg-emerald-500/10 border-emerald-500/30' },
    { id: 'email', label: 'Email', icon: Mail, color: 'text-amber-600 bg-amber-500/10 border-amber-500/30' },
    { id: 'facebook', label: 'Facebook / Messenger', icon: Share2, color: 'text-indigo-600 bg-indigo-500/10 border-indigo-500/30' }
  ];

  const CONDITION_OPTIONS = [
    { id: 'ALWAYS', label: 'Luôn gửi (Mặc định)', desc: 'Gửi vô điều kiện khi đến thời gian' },
    { id: 'IF_NOT_REPLIED', label: 'Nếu chưa phản hồi', desc: 'Dừng chuỗi nếu khách đã nhắn tin lại' },
    { id: 'IF_DEAL_STAGE_IS', label: 'Theo trạng thái Deal', desc: 'Chỉ gửi khi Lead ở giai đoạn cụ thể' },
    { id: 'IF_INTENT_IS', label: 'Theo ý định AI', desc: 'Chỉ gửi khi AI nhận diện ý định tương ứng' }
  ];

  const DELAY_PRESETS = [
    { label: 'Ngay lập tức', minutes: 0 },
    { label: '15 phút', minutes: 15 },
    { label: '1 giờ', minutes: 60 },
    { label: '4 giờ', minutes: 240 },
    { label: '1 ngày', minutes: 1440 },
    { label: '2 ngày', minutes: 2880 },
    { label: '3 ngày', minutes: 4320 },
    { label: '5 ngày', minutes: 7200 }
  ];

  const PLACEHOLDERS = [
    { token: '{{name}}', label: 'Tên khách' },
    { token: '{{phone}}', label: 'Số điện thoại' },
    { token: '{{company}}', label: 'Công ty' },
    { token: '{{email}}', label: 'Email' },
    { token: '{{value}}', label: 'Giá trị' }
  ];

  function addStep() {
    const nextOrder = steps.length + 1;
    const defaultChannel = nextOrder === 1 ? 'zalo' : (steps[nextOrder - 2]?.channel || 'whatsapp');
    const defaultDelay = nextOrder === 1 ? 0 : 1440; // Step 1 immediate, Step 2+ 1 day later
    const defaultCondition = nextOrder === 1 ? 'ALWAYS' : 'IF_NOT_REPLIED';

    steps = [
      ...steps,
      {
        step_order: nextOrder,
        channel: defaultChannel,
        delay_minutes: defaultDelay,
        condition_type: defaultCondition,
        template_content: nextOrder === 1 
          ? '{Xin chào|Chào bạn} {{name}}, bên mình thấy {{company}} đang quan tâm đến giải pháp hỗ trợ bán hàng...'
          : '{Dạ chào|Chào} anh/chị {{name}}, hôm trước bên em có gửi thông tin qua, không biết {{company}} đã xem qua chưa ạ?',
        ai_prompt_override: null
      }
    ];
  }

  function removeStep(index: number) {
    steps = steps.filter((_, i) => i !== index).map((s, i) => ({ ...s, step_order: i + 1 }));
  }

  function moveStepUp(index: number) {
    if (index <= 0) return;
    const newSteps = [...steps];
    const temp = newSteps[index];
    newSteps[index] = newSteps[index - 1];
    newSteps[index - 1] = temp;
    steps = newSteps.map((s, i) => ({ ...s, step_order: i + 1 }));
  }

  function moveStepDown(index: number) {
    if (index >= steps.length - 1) return;
    const newSteps = [...steps];
    const temp = newSteps[index];
    newSteps[index] = newSteps[index + 1];
    newSteps[index + 1] = temp;
    steps = newSteps.map((s, i) => ({ ...s, step_order: i + 1 }));
  }

  function insertPlaceholder(stepIndex: number, token: string) {
    const step = steps[stepIndex];
    if (!step) return;
    step.template_content = (step.template_content || '') + ' ' + token;
  }

  function insertSpintaxSample(stepIndex: number) {
    const step = steps[stepIndex];
    if (!step) return;
    step.template_content = '{Xin chào|Chào bạn|Kính chào} {{name}}, ' + (step.template_content || '');
  }

  function parseSpintaxPreview(text: string): string {
    let result = text
      .replace(/\{\{name\}\}/g, 'Nguyễn Văn A')
      .replace(/\{\{phone\}\}/g, '0987654321')
      .replace(/\{\{company\}\}/g, 'Công ty Công Nghệ Á Châu')
      .replace(/\{\{email\}\}/g, 'nguyenvana@gmail.com')
      .replace(/\{\{value\}\}/g, '15,000,000 VND');

    const spintaxRegex = /\{([^{}]+)\}/g;
    while (spintaxRegex.test(result)) {
      result = result.replace(spintaxRegex, (_, choices) => {
        const options = choices.split('|');
        return options[Math.floor(Math.random() * options.length)] || '';
      });
    }
    return result;
  }

  function openPreview(stepIndex: number) {
    activePreviewStepIndex = stepIndex;
    const step = steps[stepIndex];
    previewSampleOutput = step ? parseSpintaxPreview(step.template_content) : '';
    previewModalOpen = true;
  }

  function refreshPreview() {
    if (activePreviewStepIndex === null) return;
    const step = steps[activePreviewStepIndex];
    previewSampleOutput = step ? parseSpintaxPreview(step.template_content) : '';
  }

  function formatDelayText(minutes: number): string {
    if (minutes <= 0) return 'Ngay khi bắt đầu';
    if (minutes < 60) return `Sau ${minutes} phút`;
    if (minutes < 1440) {
      const hours = (minutes / 60).toFixed(1).replace('.0', '');
      return `Sau ${hours} giờ`;
    }
    const days = (minutes / 1440).toFixed(1).replace('.0', '');
    return `Sau ${days} ngày`;
  }
</script>

<div class="flex flex-col gap-4">
  <!-- Top Bar: Header & Add Step Button -->
  <div class="flex items-center justify-between gap-3">
    <div>
      <h3 class="text-sm font-semibold text-foreground flex items-center gap-2">
        <span>Kịch Bản Chuỗi Tiếp Cận (Sequence Steps)</span>
        <Badge variant="secondary" class="text-xs font-mono">{steps.length} bước</Badge>
      </h3>
      <p class="text-xs text-muted-foreground mt-0.5">
        Hệ thống tự động điều hướng kênh gửi, kiểm tra phản hồi và tối ưu tỷ lệ chuyển đổi.
      </p>
    </div>

    {#if !readonly}
      <Button
        type="button"
        size="sm"
        onclick={addStep}
        class="flex items-center gap-1.5 font-medium shadow-xs"
      >
        <Plus class="w-4 h-4" />
        <span>Thêm Bước Mới</span>
      </Button>
    {/if}
  </div>

  {#if steps.length === 0}
    <div class="p-8 border border-dashed border-border rounded-xl bg-card/50 flex flex-col items-center justify-center text-center">
      <div class="w-12 h-12 rounded-full bg-primary/10 text-primary flex items-center justify-center mb-3">
        <Sparkles class="w-6 h-6" />
      </div>
      <h4 class="font-medium text-sm text-foreground mb-1">Chưa có bước nào trong kịch bản</h4>
      <p class="text-xs text-muted-foreground max-w-sm mb-4">
        Tạo bước đầu tiên để gửi tin nhắn chào hàng qua Zalo hoặc WhatsApp, sau đó thiết lập tự động chăm sóc nếu khách chưa trả lời.
      </p>
      {#if !readonly}
        <Button type="button" size="sm" onclick={addStep} class="flex items-center gap-1.5">
          <Plus class="w-4 h-4" />
          <span>Tạo Bước Đầu Tiên</span>
        </Button>
      {/if}
    </div>
  {:else}
    <!-- Step Cards List -->
    <div class="flex flex-col gap-4">
      {#each steps as step, index (index)}
        {@const channelInfo = CHANNEL_OPTIONS.find(c => c.id === step.channel) || CHANNEL_OPTIONS[0]}
        {@const ChannelIcon = channelInfo.icon}
        <div class="border border-border bg-card rounded-xl shadow-2xs overflow-hidden transition-all hover:border-border/80">
          <!-- Step Header -->
          <div class="px-4 py-3 bg-muted/40 border-b border-border/60 flex items-center justify-between gap-3">
            <div class="flex items-center gap-2.5">
              <span class="w-6 h-6 rounded-full bg-primary text-primary-foreground text-xs font-bold flex items-center justify-center shrink-0">
                {step.step_order}
              </span>
              <span class="font-semibold text-xs text-foreground">
                Bước {step.step_order}
              </span>
              <span class="text-xs text-muted-foreground">•</span>
              <span class="text-xs font-medium px-2 py-0.5 rounded-md border flex items-center gap-1.5 {channelInfo.color}">
                <ChannelIcon class="w-3.5 h-3.5" />
                <span>{channelInfo.label}</span>
              </span>
              <span class="text-xs text-muted-foreground flex items-center gap-1">
                <Clock class="w-3 h-3" />
                <span>{formatDelayText(step.delay_minutes)}</span>
              </span>
            </div>

            <!-- Controls (Reorder / Delete) -->
            {#if !readonly}
              <div class="flex items-center gap-1">
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  class="h-7 w-7 text-muted-foreground hover:text-foreground"
                  disabled={index === 0}
                  onclick={() => moveStepUp(index)}
                  title="Di chuyển lên"
                >
                  <ArrowUp class="w-3.5 h-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  class="h-7 w-7 text-muted-foreground hover:text-foreground"
                  disabled={index === steps.length - 1}
                  onclick={() => moveStepDown(index)}
                  title="Di chuyển xuống"
                >
                  <ArrowDown class="w-3.5 h-3.5" />
                </Button>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon"
                  class="h-7 w-7 text-rose-500 hover:text-rose-600 hover:bg-rose-500/10"
                  onclick={() => removeStep(index)}
                  title="Xóa bước này"
                >
                  <Trash2 class="w-3.5 h-3.5" />
                </Button>
              </div>
            {/if}
          </div>

          <!-- Step Body -->
          <div class="p-4 flex flex-col gap-4">
            <!-- Row 1: Channel Selector & Delay & Condition -->
            <div class="grid grid-cols-1 md:grid-cols-3 gap-3">
              <!-- Channel -->
              <div class="flex flex-col gap-1.5">
                <label for={`channel-step-${index}`} class="text-xs font-medium text-muted-foreground">Kênh Tiếp Cận</label>
                <select
                  id={`channel-step-${index}`}
                  bind:value={step.channel}
                  disabled={readonly}
                  class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                >
                  {#each CHANNEL_OPTIONS as ch}
                    <option value={ch.id}>{ch.label}</option>
                  {/each}
                </select>
              </div>

              <!-- Delay Preset / Custom -->
              <div class="flex flex-col gap-1.5">
                <label for={`delay-step-${index}`} class="text-xs font-medium text-muted-foreground flex items-center justify-between">
                  <span>Khoảng Chờ (Delay)</span>
                  <span class="font-mono text-[11px] text-primary">{step.delay_minutes} phút</span>
                </label>
                <select
                  id={`delay-step-${index}`}
                  bind:value={step.delay_minutes}
                  disabled={readonly}
                  class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                >
                  {#each DELAY_PRESETS as preset}
                    <option value={preset.minutes}>{preset.label} ({preset.minutes}p)</option>
                  {/each}
                </select>
              </div>

              <!-- Condition -->
              <div class="flex flex-col gap-1.5">
                <label for={`cond-step-${index}`} class="text-xs font-medium text-muted-foreground">Điều Kiện Kích Hoạt</label>
                <select
                  id={`cond-step-${index}`}
                  bind:value={step.condition_type}
                  disabled={readonly}
                  class="h-9 px-3 text-xs rounded-lg border border-input bg-background text-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                >
                  {#each CONDITION_OPTIONS as cond}
                    <option value={cond.id}>{cond.label}</option>
                  {/each}
                </select>
              </div>
            </div>

            <!-- Row 2: Template Editor & Spintax / Placeholders -->
            <div class="flex flex-col gap-2">
              <div class="flex items-center justify-between gap-2 flex-wrap">
                <div class="flex items-center gap-2">
                  <label for={`template-step-${index}`} class="text-xs font-medium text-muted-foreground">Nội Dung Tin Nhắn Mẫu</label>
                  {#if step.template_id}
                    <Badge variant="outline" class="text-[10px] text-primary border-primary/30 flex items-center gap-1 font-mono">
                      <BookOpen class="w-3 h-3" />
                      <span>Mẫu #{step.template_id}</span>
                    </Badge>
                  {/if}
                </div>

                {#if !readonly}
                  <div class="flex items-center gap-2 flex-wrap">
                    <!-- Template Library Integration Buttons -->
                    <div class="flex items-center gap-1">
                      <button
                        type="button"
                        onclick={() => openPickerForStep(index)}
                        class="text-[11px] px-2.5 py-1 rounded-md bg-primary/10 hover:bg-primary/20 text-primary border border-primary/30 transition-colors font-medium flex items-center gap-1.5 cursor-pointer shadow-2xs"
                        title="Chọn mẫu từ Thư Viện Mẫu tập trung"
                      >
                        <FolderOpen class="w-3.5 h-3.5" />
                        <span>Chọn từ Thư Viện Mẫu</span>
                      </button>

                      <button
                        type="button"
                        onclick={() => openSaveForStep(index)}
                        class="text-[11px] px-2.5 py-1 rounded-md bg-muted hover:bg-muted/80 text-foreground border border-border transition-colors font-medium flex items-center gap-1.5 cursor-pointer"
                        title="Lưu nội dung bước này vào Thư Viện Mẫu"
                      >
                        <Save class="w-3.5 h-3.5" />
                        <span>Lưu thành Mẫu</span>
                      </button>
                    </div>

                    <div class="h-3.5 w-px bg-border/70 mx-0.5"></div>

                    <!-- Variables & Spintax -->
                    <div class="flex items-center gap-1 flex-wrap">
                      <span class="text-[11px] text-muted-foreground">Biến:</span>
                      {#each PLACEHOLDERS as p}
                        <button
                          type="button"
                          onclick={() => insertPlaceholder(index, p.token)}
                          class="text-[11px] px-2 py-0.5 rounded bg-muted hover:bg-muted/80 text-foreground border border-border transition-colors font-mono cursor-pointer"
                          title={`Chèn ${p.label}`}
                        >
                          {p.token}
                        </button>
                      {/each}

                      <button
                        type="button"
                        onclick={() => insertSpintaxSample(index)}
                        class="text-[11px] px-2 py-0.5 rounded bg-primary/10 hover:bg-primary/20 text-primary border border-primary/20 transition-colors font-medium flex items-center gap-1 cursor-pointer"
                      >
                        <Sparkles class="w-3 h-3" />
                        <span>+ Spintax</span>
                      </button>
                    </div>
                  </div>
                {/if}
              </div>

              <textarea
                id={`template-step-${index}`}
                bind:value={step.template_content}
                disabled={readonly}
                rows={4}
                placeholder={'Nhập nội dung tin nhắn mẫu. Hỗ trợ Spintax dạng {Xin chào|Chào bạn} và biến CRM dạng {{name}}, {{company}}...'}
                class="w-full p-3 rounded-lg border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring font-sans leading-relaxed resize-y"
              ></textarea>

              <div class="flex items-center justify-between text-xs text-muted-foreground">
                <span class="text-[11px]">
                  💡 Dùng Spintax <code>{'{A|B|C}'}</code> để giảm thiểu nguy cơ bị chặn/spam từ mạng xã hội.
                </span>
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  onclick={() => openPreview(index)}
                  class="h-7 text-xs flex items-center gap-1.5"
                >
                  <Eye class="w-3.5 h-3.5" />
                  <span>Xem Thử Ngẫu Nhiên</span>
                </Button>
              </div>
            </div>

            <!-- Row 3: AI Prompt Override (Optional Accordion) -->
            <div class="pt-2 border-t border-border/50">
              <details class="group">
                <summary class="text-xs font-medium text-muted-foreground hover:text-foreground cursor-pointer flex items-center justify-between list-none py-1">
                  <div class="flex items-center gap-1.5">
                    <Bot class="w-3.5 h-3.5 text-primary" />
                    <span>Tùy chỉnh AI Copilot / Generative Prompt (Tùy chọn)</span>
                  </div>
                  <span class="text-[10px] text-muted-foreground group-open:rotate-180 transition-transform">▼</span>
                </summary>
                <div class="mt-2 pt-2 flex flex-col gap-2">
                  <p class="text-[11px] text-muted-foreground">
                    Nếu bạn muốn AI tự động sinh nội dung riêng biệt cho từng Lead ở bước này, hãy nhập chỉ dẫn prompt bên dưới.
                  </p>
                  <input
                    type="text"
                    bind:value={step.ai_prompt_override}
                    disabled={readonly}
                    placeholder="Ví dụ: Đóng vai chuyên viên tư vấn phần mềm, nhấn mạnh ưu đãi giảm 20% cho ngành của công ty khách hàng..."
                    class="w-full h-8 px-3 rounded-md border border-input bg-background text-xs text-foreground placeholder:text-muted-foreground focus:outline-hidden focus:ring-1 focus:ring-ring"
                  />
                </div>
              </details>
            </div>
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

<!-- Spintax & Placeholder Preview Modal -->
{#if previewModalOpen}
  <div class="fixed inset-0 z-50 bg-black/60 flex items-center justify-center p-4 backdrop-blur-xs animate-fade-in">
    <div class="bg-card border border-border rounded-xl max-w-lg w-full p-6 shadow-xl flex flex-col gap-4">
      <div class="flex items-center justify-between">
        <h3 class="text-base font-semibold text-foreground flex items-center gap-2">
          <Eye class="w-4 h-4 text-primary" />
          <span>Mô Phỏng Tin Nhắn Sau Xử Lý</span>
        </h3>
        <Button variant="ghost" size="sm" onclick={() => previewModalOpen = false}>Đóng</Button>
      </div>

      <p class="text-xs text-muted-foreground">
        Hệ thống tự động thay thế các biến CRM và chọn ngẫu nhiên từ khóa trong Spintax mỗi khi gửi tin.
      </p>

      <div class="p-4 rounded-lg bg-muted/50 border border-border text-xs text-foreground font-sans leading-relaxed whitespace-pre-wrap">
        {previewSampleOutput || '(Nội dung trống)'}
      </div>

      <div class="flex items-center justify-between pt-2">
        <Button variant="outline" size="sm" onclick={refreshPreview} class="flex items-center gap-1.5 text-xs">
          <Sparkles class="w-3.5 h-3.5 text-primary" />
          <span>Thử Biến Thể Khác (Randomize)</span>
        </Button>
        <Button size="sm" onclick={() => previewModalOpen = false}>Hoàn Tất</Button>
      </div>
    </div>
  </div>
{/if}

<!-- Template Picker Modal -->
<TemplatePickerModal
  bind:open={pickerModalOpen}
  channel={pickerChannel}
  onSelect={handleTemplateSelected}
/>

<!-- Save Template Modal -->
<SaveTemplateModal
  bind:open={saveModalOpen}
  initialContent={saveInitialContent}
  initialChannel={saveInitialChannel}
  onSaved={handleTemplateSaved}
/>
