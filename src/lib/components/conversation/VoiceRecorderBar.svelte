<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { Button } from '$lib/components/ui/button';
  import { Mic, Square, Trash2, Send } from 'lucide-svelte';

  let {
    onRecordingComplete = () => {},
    onCancel = () => {}
  }: {
    onRecordingComplete: (file: File) => void;
    onCancel: () => void;
  } = $props();

  let mediaRecorder = $state<MediaRecorder | null>(null);
  let audioChunks = $state<Blob[]>([]);
  let isRecording = $state(false);
  let recordingSeconds = $state(0);
  let timerInterval: ReturnType<typeof setInterval> | null = null;
  let errorMessage = $state<string | null>(null);
  let unmounted = false;

  onMount(() => {
    unmounted = false;
    void startRecording();
  });

  onDestroy(() => {
    unmounted = true;
    stopTimer();
    if (mediaRecorder && mediaRecorder.state !== 'inactive') {
      mediaRecorder.stop();
      mediaRecorder.stream.getTracks().forEach((track) => track.stop());
    }
  });

  function startTimer() {
    stopTimer();
    recordingSeconds = 0;
    timerInterval = setInterval(() => {
      recordingSeconds += 1;
    }, 1000);
  }

  function stopTimer() {
    if (timerInterval) {
      clearInterval(timerInterval);
      timerInterval = null;
    }
  }

  function formatDuration(seconds: number): string {
    const mins = Math.floor(seconds / 60);
    const secs = seconds % 60;
    return `${String(mins).padStart(2, '0')}:${String(secs).padStart(2, '0')}`;
  }

  async function startRecording() {
    try {
      errorMessage = null;
      const stream = await navigator.mediaDevices.getUserMedia({ audio: true });
      if (unmounted) {
        stream.getTracks().forEach((track) => track.stop());
        return;
      }
      audioChunks = [];
      const recorder = new MediaRecorder(stream);

      recorder.ondataavailable = (event) => {
        if (event.data.size > 0) {
          audioChunks.push(event.data);
        }
      };

      recorder.onstop = () => {
        stopTimer();
        stream.getTracks().forEach((track) => track.stop());
      };

      recorder.start();
      mediaRecorder = recorder;
      isRecording = true;
      startTimer();
    } catch (err) {
      if (!unmounted) {
        errorMessage = err instanceof Error ? err.message : 'Unable to access microphone';
      }
    }
  }

  function handleCancel() {
    unmounted = true;
    if (mediaRecorder && mediaRecorder.state !== 'inactive') {
      mediaRecorder.stop();
    }
    stopTimer();
    onCancel();
  }

  function handleSend() {
    if (!mediaRecorder || mediaRecorder.state === 'inactive') return;

    mediaRecorder.onstop = () => {
      stopTimer();
      mediaRecorder?.stream.getTracks().forEach((track) => track.stop());
      const audioBlob = new Blob(audioChunks, { type: 'audio/webm' });
      const file = new File([audioBlob], `voice_note_${Date.now()}.webm`, {
        type: 'audio/webm'
      });
      onRecordingComplete(file);
    };

    mediaRecorder.stop();
  }
</script>

<div class="flex items-center justify-between gap-3 p-2 bg-destructive/10 text-destructive rounded-lg border border-destructive/20 text-sm animate-in fade-in slide-in-from-bottom-1">
  <div class="flex items-center gap-2 font-medium">
    <span class="relative flex size-3">
      <span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-destructive opacity-75"></span>
      <span class="relative inline-flex size-3 rounded-full bg-destructive"></span>
    </span>
    <Mic class="size-4 animate-pulse" />
    <span>Recording ({formatDuration(recordingSeconds)})</span>
  </div>

  {#if errorMessage}
    <span class="text-xs text-destructive font-normal">{errorMessage}</span>
  {/if}

  <div class="flex items-center gap-1.5">
    <Button
      type="button"
      variant="ghost"
      size="icon"
      class="size-7 text-destructive hover:bg-destructive/20"
      title="Cancel recording"
      onclick={handleCancel}
    >
      <Trash2 class="size-4" />
    </Button>
    <Button
      type="button"
      variant="destructive"
      size="sm"
      class="h-7 px-3 text-xs gap-1 font-medium"
      onclick={handleSend}
    >
      <Send class="size-3.5" />
      <span>Send voice</span>
    </Button>
  </div>
</div>
