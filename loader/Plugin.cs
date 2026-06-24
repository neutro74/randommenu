using System;
using System.Collections;
using System.Collections.Generic;
using System.IO;
using System.Net;
using System.Runtime.InteropServices;
using BepInEx;
using TMPro;
using UnityEngine;
using UnityEngine.XR;

namespace RandomMenuLoader
{
    [BepInPlugin("com.neutro74.randommenu", "randommenu", "1.0.0")]
    public class Plugin : BaseUnityPlugin
    {
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_init();
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_tick(uint bitmask);
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern uint menu_load_saved();
        [DllImport("randommenu", CallingConvention = CallingConvention.Cdecl)]
        static extern void menu_save(uint bitmask);

        const string DLL_URL = "https://github.com/neutro74/randommenu/releases/latest/download/randommenu.dll";

        static readonly string[] ModNames = { "Speed Boost", "Fly", "Long Arms", "Freeze Self", "Ghost", "Bounce" };

        static readonly Color BG_DARK    = new Color(0.04f, 0.04f, 0.06f, 0.92f);
        static readonly Color BG_BTN     = new Color(0.10f, 0.10f, 0.14f, 1f);
        static readonly Color COL_ON     = new Color(0.18f, 0.85f, 0.40f, 1f);
        static readonly Color COL_OFF    = new Color(0.55f, 0.55f, 0.60f, 1f);
        static readonly Color COL_ACCENT = new Color(0.30f, 0.70f, 1.00f, 1f);
        static readonly Color COL_WHITE  = new Color(0.95f, 0.95f, 1.00f, 1f);

        uint  enabledBitmask = 0;
        bool  menuOpen       = false;
        bool  yWasDown       = false;
        float buttonCooldown = 0f;
        bool  menuDllReady   = false;

        // hand trackers: non-trigger colliders with kinematic Rigidbodies
        // they enter button trigger zones → OnTriggerEnter fires on the zone
        Collider    leftHandTracker  = null;
        Collider    rightHandTracker = null;
        GameObject  leftTrackerGO   = null;
        GameObject  rightTrackerGO  = null;

        GameObject menuRoot     = null;
        Renderer[] btnRenderers = null;

        void Awake()
        {
            StartCoroutine(InitRoutine());
        }

        IEnumerator InitRoutine()
        {
            string dllPath = Path.Combine(Paths.GameRootPath, "randommenu.dll");

            Logger.LogInfo("randommenu: downloading DLL...");
            bool downloaded = false;
            try
            {
                new WebClient().DownloadFile(DLL_URL, dllPath);
                downloaded = true;
            }
            catch (Exception e)
            {
                Logger.LogWarning("randommenu: download failed: " + e.Message);
            }

            if (!downloaded && !File.Exists(dllPath))
            {
                Logger.LogError("randommenu: no DLL found, menu disabled");
                yield break;
            }

            // yield one frame so the DLL is flushed to disk before we P/Invoke
            yield return null;

            try
            {
                menu_init();
                enabledBitmask = menu_load_saved();
                menuDllReady = true;
                Logger.LogInfo("randommenu: DLL ready, saved bitmask=" + enabledBitmask);
            }
            catch (Exception e)
            {
                Logger.LogError("randommenu: menu_init failed: " + e.Message);
                yield break;
            }

            SpawnHandTrackers();
        }

        void Update()
        {
            if (!menuDllReady) return;

            if (leftTrackerGO != null)
                UpdateTrackerPositions();

            // Y button = secondaryButton on left controller
            var leftDev = InputDevices.GetDeviceAtXRNode(XRNode.LeftHand);
            leftDev.TryGetFeatureValue(CommonUsages.secondaryButton, out bool yDown);

            if (yDown && !yWasDown)
            {
                menuOpen = !menuOpen;
                Logger.LogInfo("randommenu: menu " + (menuOpen ? "opened" : "closed"));
                if (menuOpen) DrawMenu();
                else DestroyMenu();
            }
            yWasDown = yDown;

            if (menuOpen && menuRoot != null)
                PositionMenu();

            menu_tick(enabledBitmask);
        }

        void SpawnHandTrackers()
        {
            leftTrackerGO  = MakeTracker("rm_lhand");
            rightTrackerGO = MakeTracker("rm_rhand");
            leftHandTracker  = leftTrackerGO.GetComponent<SphereCollider>();
            rightHandTracker = rightTrackerGO.GetComponent<SphereCollider>();
        }

        // hand trackers are solid (non-trigger) with kinematic Rigidbody
        // this lets them generate trigger-enter events on the button trigger zones
        static GameObject MakeTracker(string name)
        {
            var go = GameObject.CreatePrimitive(PrimitiveType.Sphere);
            go.name = name;
            Destroy(go.GetComponent<Renderer>());
            go.transform.localScale = Vector3.one * 0.06f;

            // kinematic Rigidbody is required for trigger-enter events
            var rb = go.GetComponent<Rigidbody>() ?? go.AddComponent<Rigidbody>();
            rb.isKinematic = true;
            rb.useGravity  = false;

            // NOT a trigger — it is the thing that enters button trigger zones
            go.GetComponent<SphereCollider>().isTrigger = false;

            DontDestroyOnLoad(go);
            return go;
        }

        void UpdateTrackerPositions()
        {
            var leftDev  = InputDevices.GetDeviceAtXRNode(XRNode.LeftHand);
            var rightDev = InputDevices.GetDeviceAtXRNode(XRNode.RightHand);

            leftDev.TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 lp);
            leftDev.TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion lr);
            rightDev.TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 rp);
            rightDev.TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion rr);

            leftTrackerGO.transform.SetPositionAndRotation(lp, lr);
            rightTrackerGO.transform.SetPositionAndRotation(rp, rr);
        }

        void PositionMenu()
        {
            var leftDev = InputDevices.GetDeviceAtXRNode(XRNode.LeftHand);
            leftDev.TryGetFeatureValue(CommonUsages.devicePosition, out Vector3 pos);
            leftDev.TryGetFeatureValue(CommonUsages.deviceRotation, out Quaternion rot);
            menuRoot.transform.position = pos + rot * new Vector3(0f, 0.1f, 0f);
            menuRoot.transform.rotation = rot * Quaternion.Euler(0f, 0f, 90f);
        }

        void DrawMenu()
        {
            DestroyMenu();

            menuRoot    = new GameObject("rm_root");
            btnRenderers = new Renderer[ModNames.Length];

            float btnH   = 0.045f;
            float btnW   = 0.22f;
            float gap    = 0.005f;
            float titleH = 0.03f;
            float totalH = titleH + gap + ModNames.Length * (btnH + gap);
            float startZ = (totalH * 0.5f) - titleH - gap;

            // dark background slab
            var bg = MakeCube("rm_bg", menuRoot.transform);
            bg.transform.localScale    = new Vector3(0.007f, btnW + 0.01f, totalH + 0.01f);
            bg.transform.localPosition = Vector3.zero;
            bg.GetComponent<Renderer>().material.color = BG_DARK;

            // cyan accent stripe
            var stripe = MakeCube("rm_stripe", menuRoot.transform);
            stripe.transform.localScale    = new Vector3(0.008f, 0.004f, totalH + 0.01f);
            stripe.transform.localPosition = new Vector3(0f, btnW * 0.5f + 0.005f, 0f);
            stripe.GetComponent<Renderer>().material.color = COL_ACCENT;

            // title
            MakeLabel(menuRoot.transform, "randommenu", 3.2f, COL_ACCENT,
                new Vector3(0.006f, 0f, startZ + titleH * 0.5f),
                Quaternion.Euler(90f, 0f, 90f));

            // separator
            var sep = MakeCube("rm_sep", menuRoot.transform);
            sep.transform.localScale    = new Vector3(0.007f, btnW, 0.001f);
            sep.transform.localPosition = new Vector3(0f, 0f, startZ);
            sep.GetComponent<Renderer>().material.color = COL_ACCENT * 0.6f;

            for (int i = 0; i < ModNames.Length; i++)
            {
                float z  = startZ - gap - btnH * 0.5f - i * (btnH + gap);
                bool  on = (enabledBitmask & (1u << i)) != 0;

                // button background
                var btn = MakeCube("rm_btn_" + i, menuRoot.transform);
                Destroy(btn.GetComponent<BoxCollider>());
                btn.transform.localScale    = new Vector3(0.008f, btnW - 0.006f, btnH);
                btn.transform.localPosition = new Vector3(0f, 0f, z);
                btn.GetComponent<Renderer>().material.color = BG_BTN;
                btnRenderers[i] = btn.GetComponent<Renderer>();

                // status dot
                var dot = MakeCube("rm_dot_" + i, menuRoot.transform);
                Destroy(dot.GetComponent<BoxCollider>());
                dot.transform.localScale    = new Vector3(0.009f, 0.008f, btnH * 0.6f);
                dot.transform.localPosition = new Vector3(0f, -(btnW * 0.5f - 0.008f), z);
                dot.GetComponent<Renderer>().material.color = on ? COL_ON : COL_OFF;

                // label
                MakeLabel(menuRoot.transform, ModNames[i], 2.8f, COL_WHITE,
                    new Vector3(0.007f, 0.01f, z),
                    Quaternion.Euler(90f, 0f, 90f));

                // trigger zone — button fires when hand solid collider enters this
                var trigger = new GameObject("rm_trigger_" + i);
                trigger.transform.SetParent(menuRoot.transform, false);
                trigger.transform.localScale    = new Vector3(0.05f, btnW, btnH * 1.2f);
                trigger.transform.localPosition = new Vector3(0f, 0f, z);
                trigger.AddComponent<BoxCollider>().isTrigger = true;
                var handler    = trigger.AddComponent<ButtonHandler>();
                handler.plugin   = this;
                handler.modIndex = i;
            }
        }

        static GameObject MakeCube(string name, Transform parent)
        {
            var go = GameObject.CreatePrimitive(PrimitiveType.Cube);
            go.name = name;
            Destroy(go.GetComponent<Rigidbody>());
            go.transform.SetParent(parent, false);
            return go;
        }

        static void MakeLabel(Transform parent, string text, float size, Color color,
                               Vector3 localPos, Quaternion localRot)
        {
            var go = new GameObject("rm_lbl");
            go.transform.SetParent(parent, false);
            go.transform.localPosition = localPos;
            go.transform.localRotation = localRot;
            go.transform.localScale    = Vector3.one * 0.01f;

            var tmp = go.AddComponent<TextMeshPro>();
            tmp.text               = text;
            tmp.fontSize           = size;
            tmp.color              = color;
            tmp.alignment          = TextAlignmentOptions.MidlineLeft;
            tmp.fontStyle          = FontStyles.Bold;
            tmp.enableWordWrapping = false;
            tmp.overflowMode       = TextOverflowModes.Overflow;
        }

        void DestroyMenu()
        {
            if (menuRoot != null) { Destroy(menuRoot); menuRoot = null; btnRenderers = null; }
        }

        public void OnButtonPressed(int modIndex)
        {
            if (Time.time < buttonCooldown) return;
            buttonCooldown = Time.time + 0.25f;

            enabledBitmask ^= (1u << modIndex);
            menu_save(enabledBitmask);
            Logger.LogInfo("randommenu: toggled mod " + modIndex + " → " + ((enabledBitmask >> modIndex & 1) == 1 ? "ON" : "OFF"));

            if (btnRenderers != null && modIndex < btnRenderers.Length && btnRenderers[modIndex] != null)
            {
                bool on = (enabledBitmask & (1u << modIndex)) != 0;
                Transform dot = btnRenderers[modIndex].transform.parent.Find("rm_dot_" + modIndex);
                if (dot != null)
                    dot.GetComponent<Renderer>().material.color = on ? COL_ON : COL_OFF;
            }
        }

        public bool IsHandCollider(Collider c) =>
            c == leftHandTracker || c == rightHandTracker;

        void OnDestroy()
        {
            DestroyMenu();
            if (leftTrackerGO  != null) Destroy(leftTrackerGO);
            if (rightTrackerGO != null) Destroy(rightTrackerGO);
        }
    }

    class ButtonHandler : MonoBehaviour
    {
        public Plugin plugin;
        public int    modIndex;

        void OnTriggerEnter(Collider other)
        {
            if (plugin != null && plugin.IsHandCollider(other))
                plugin.OnButtonPressed(modIndex);
        }
    }
}
