#!/usr/bin/env python3
import os
import plistlib
import uuid

def gen_id():
    return uuid.uuid4().hex[:24].upper()

def main():
    proj_dir = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "apps", "macos"))
    xcodeproj_dir = os.path.join(proj_dir, "Vacua.xcodeproj")
    os.makedirs(xcodeproj_dir, exist_ok=True)
    shared_schemes_dir = os.path.join(xcodeproj_dir, "xcshareddata", "xcschemes")
    os.makedirs(shared_schemes_dir, exist_ok=True)

    # IDs
    project_id = gen_id()
    main_group_id = gen_id()
    products_group_id = gen_id()
    vacua_group_id = gen_id()
    vacua_tests_group_id = gen_id()
    vacua_uitests_group_id = gen_id()

    vacua_target_id = gen_id()
    vacua_tests_target_id = gen_id()
    vacua_uitests_target_id = gen_id()

    vacua_app_ref_id = gen_id()
    vacua_tests_ref_id = gen_id()
    vacua_uitests_ref_id = gen_id()

    package_ref_id = gen_id()
    package_product_vacua_id = gen_id()
    package_product_tests_id = gen_id()

    # Build phases IDs
    vacua_sources_id = gen_id()
    vacua_frameworks_id = gen_id()
    vacua_resources_id = gen_id()

    tests_sources_id = gen_id()
    tests_frameworks_id = gen_id()
    tests_resources_id = gen_id()

    uitests_sources_id = gen_id()
    uitests_frameworks_id = gen_id()
    uitests_resources_id = gen_id()

    # Configs
    proj_cfg_list_id = gen_id()
    proj_cfg_debug_id = gen_id()
    proj_cfg_release_id = gen_id()

    vacua_cfg_list_id = gen_id()
    vacua_cfg_debug_id = gen_id()
    vacua_cfg_release_id = gen_id()

    tests_cfg_list_id = gen_id()
    tests_cfg_debug_id = gen_id()
    tests_cfg_release_id = gen_id()

    uitests_cfg_list_id = gen_id()
    uitests_cfg_debug_id = gen_id()
    uitests_cfg_release_id = gen_id()

    # Files
    app_files = [
        ("Vacua/App/VacuaApp.swift", "VacuaApp.swift"),
        ("Vacua/App/AppModel.swift", "AppModel.swift"),
        ("Vacua/Common/DesignSystem/VacuaTheme.swift", "VacuaTheme.swift"),
        ("Vacua/Common/DesignSystem/VacuaSymbols.swift", "VacuaSymbols.swift"),
        ("Vacua/Common/DesignSystem/Metrics.swift", "Metrics.swift"),
        ("Vacua/Common/DesignSystem/Badges.swift", "Badges.swift"),
        ("Vacua/Common/DesignSystem/StatusViews.swift", "StatusViews.swift"),
        ("Vacua/Common/DesignSystem/EmptyState.swift", "EmptyState.swift"),
        ("Vacua/Common/DesignSystem/ErrorState.swift", "ErrorState.swift"),
        ("Vacua/Features/Overview/OverviewView.swift", "OverviewView.swift"),
        ("Vacua/Features/Candidates/CandidatesView.swift", "CandidatesView.swift"),
        ("Vacua/Features/Candidates/CandidateDetailView.swift", "CandidateDetailView.swift"),
        ("Vacua/Features/Candidates/CleanupSimulationSheet.swift", "CleanupSimulationSheet.swift"),
        ("Vacua/Features/Candidates/CleanupProposalSheet.swift", "CleanupProposalSheet.swift"),
        ("Vacua/Features/Duplicates/DuplicatesView.swift", "DuplicatesView.swift"),
        ("Vacua/Features/Applications/ApplicationsView.swift", "ApplicationsView.swift"),
        ("Vacua/Features/DeveloperArtifacts/DeveloperArtifactsModel.swift", "DeveloperArtifactsModel.swift"),
        ("Vacua/Features/DeveloperArtifacts/DeveloperArtifactsView.swift", "DeveloperArtifactsView.swift"),
        ("Vacua/Features/Snapshots/SnapshotsView.swift", "SnapshotsView.swift"),
        ("Vacua/Features/Settings/SettingsView.swift", "SettingsView.swift"),
        ("Vacua/Features/StorageMap/StorageMapModel.swift", "StorageMapModel.swift"),
        ("Vacua/Features/StorageMap/StorageMapToolbar.swift", "StorageMapToolbar.swift"),
        ("Vacua/Features/StorageMap/StorageMapView.swift", "StorageMapView.swift"),
        ("Vacua/Features/StorageMap/StorageNodeInspector.swift", "StorageNodeInspector.swift"),
        ("Vacua/Features/StorageMap/TreemapLayout.swift", "TreemapLayout.swift"),
        ("Vacua/Features/StorageMap/TreemapView.swift", "TreemapView.swift"),
    ]

    test_files = [
        ("VacuaTests/AppModelTests.swift", "AppModelTests.swift"),
        ("VacuaTests/TreemapLayoutTests.swift", "TreemapLayoutTests.swift"),
    ]

    uitest_files = [
        ("VacuaUITests/VacuaUITests.swift", "VacuaUITests.swift"),
    ]

    info_plist_id = gen_id()
    assets_ref_id = gen_id()
    assets_bf_id = gen_id()
    localizable_ref_id = gen_id()
    localizable_bf_id = gen_id()

    # Build File and File Ref dicts
    app_file_entries = []
    for rel_path, name in app_files:
        f_id = gen_id()
        bf_id = gen_id()
        app_file_entries.append((f_id, bf_id, rel_path, name))

    test_file_entries = []
    for rel_path, name in test_files:
        f_id = gen_id()
        bf_id = gen_id()
        test_file_entries.append((f_id, bf_id, rel_path, name))

    uitest_file_entries = []
    for rel_path, name in uitest_files:
        f_id = gen_id()
        bf_id = gen_id()
        uitest_file_entries.append((f_id, bf_id, rel_path, name))

    pbx = f"""// !$*UTF8*$!
{{
	archiveVersion = 1;
	classes = {{
	}};
	objectVersion = 77;
	objects = {{

/* Begin PBXBuildFile section */
"""
    for f_id, bf_id, rel_path, name in app_file_entries:
        pbx += f"\t\t{bf_id} /* {name} in Sources */ = {{isa = PBXBuildFile; fileRef = {f_id} /* {name} */; }};\n"
    for f_id, bf_id, rel_path, name in test_file_entries:
        pbx += f"\t\t{bf_id} /* {name} in Sources */ = {{isa = PBXBuildFile; fileRef = {f_id} /* {name} */; }};\n"
    for f_id, bf_id, rel_path, name in uitest_file_entries:
        pbx += f"\t\t{bf_id} /* {name} in Sources */ = {{isa = PBXBuildFile; fileRef = {f_id} /* {name} */; }};\n"

    vacua_pkg_bf_id = gen_id()
    tests_pkg_bf_id = gen_id()
    pbx += f"\t\t{vacua_pkg_bf_id} /* VacuaClient in Frameworks */ = {{isa = PBXBuildFile; productRef = {package_product_vacua_id} /* VacuaClient */; }};\n"
    pbx += f"\t\t{tests_pkg_bf_id} /* VacuaClient in Frameworks */ = {{isa = PBXBuildFile; productRef = {package_product_tests_id} /* VacuaClient */; }};\n"
    pbx += f"\t\t{assets_bf_id} /* Assets.xcassets in Resources */ = {{isa = PBXBuildFile; fileRef = {assets_ref_id} /* Assets.xcassets */; }};\n"
    pbx += f"\t\t{localizable_bf_id} /* Localizable.xcstrings in Resources */ = {{isa = PBXBuildFile; fileRef = {localizable_ref_id} /* Localizable.xcstrings */; }};\n"

    pbx += """/* End PBXBuildFile section */

/* Begin PBXContainerItemProxy section */
"""
    target_proxy_tests_id = gen_id()
    target_proxy_uitests_id = gen_id()
    target_dep_tests_id = gen_id()
    target_dep_uitests_id = gen_id()
    pbx += f"""\t\t{target_proxy_tests_id} /* PBXContainerItemProxy */ = {{
\t\t\tisa = PBXContainerItemProxy;
\t\t\tcontainerPortal = {project_id} /* Project object */;
\t\t\tproxyType = 1;
\t\t\tremoteGlobalIDString = {vacua_target_id};
\t\t\tremoteInfo = Vacua;
\t\t}};
\t\t{target_proxy_uitests_id} /* PBXContainerItemProxy */ = {{
\t\t\tisa = PBXContainerItemProxy;
\t\t\tcontainerPortal = {project_id} /* Project object */;
\t\t\tproxyType = 1;
\t\t\tremoteGlobalIDString = {vacua_target_id};
\t\t\tremoteInfo = Vacua;
\t\t}};
/* End PBXContainerItemProxy section */

/* Begin PBXTargetDependency section */
\t\t{target_dep_tests_id} /* PBXTargetDependency */ = {{
\t\t\tisa = PBXTargetDependency;
\t\t\ttarget = {vacua_target_id} /* Vacua */;
\t\t\ttargetProxy = {target_proxy_tests_id} /* PBXContainerItemProxy */;
\t\t}};
\t\t{target_dep_uitests_id} /* PBXTargetDependency */ = {{
\t\t\tisa = PBXTargetDependency;
\t\t\ttarget = {vacua_target_id} /* Vacua */;
\t\t\ttargetProxy = {target_proxy_uitests_id} /* PBXContainerItemProxy */;
\t\t}};
/* End PBXTargetDependency section */

/* Begin PBXFileReference section */
\t\t{vacua_app_ref_id} /* Vacua.app */ = {{isa = PBXFileReference; explicitFileType = wrapper.application; includeInIndex = 0; path = Vacua.app; sourceTree = BUILT_PRODUCTS_DIR; }};
\t\t{vacua_tests_ref_id} /* VacuaTests.xctest */ = {{isa = PBXFileReference; explicitFileType = wrapper.cfbundle; includeInIndex = 0; path = VacuaTests.xctest; sourceTree = BUILT_PRODUCTS_DIR; }};
\t\t{vacua_uitests_ref_id} /* VacuaUITests.xctest */ = {{isa = PBXFileReference; explicitFileType = wrapper.cfbundle; includeInIndex = 0; path = VacuaUITests.xctest; sourceTree = BUILT_PRODUCTS_DIR; }};
\t\t{info_plist_id} /* Info.plist */ = {{isa = PBXFileReference; lastKnownFileType = text.plist.xml; path = "Vacua/Resources/Info.plist"; sourceTree = "<group>"; }};
\t\t{assets_ref_id} /* Assets.xcassets */ = {{isa = PBXFileReference; lastKnownFileType = folder.assetcatalog; path = "Vacua/Resources/Assets.xcassets"; sourceTree = "<group>"; }};
\t\t{localizable_ref_id} /* Localizable.xcstrings */ = {{isa = PBXFileReference; lastKnownFileType = text.json.xcstrings; path = "Vacua/Resources/Localizable.xcstrings"; sourceTree = "<group>"; }};
"""
    for f_id, bf_id, rel_path, name in app_file_entries:
        pbx += f"\t\t{f_id} /* {name} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = \"{rel_path}\"; sourceTree = \"<group>\"; }};\n"
    for f_id, bf_id, rel_path, name in test_file_entries:
        pbx += f"\t\t{f_id} /* {name} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = \"{rel_path}\"; sourceTree = \"<group>\"; }};\n"
    for f_id, bf_id, rel_path, name in uitest_file_entries:
        pbx += f"\t\t{f_id} /* {name} */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = \"{rel_path}\"; sourceTree = \"<group>\"; }};\n"

    pbx += """/* End PBXFileReference section */

/* Begin PBXFrameworksBuildPhase section */
"""
    pbx += f"""\t\t{vacua_frameworks_id} /* Frameworks */ = {{
\t\t\tisa = PBXFrameworksBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t\t{vacua_pkg_bf_id} /* VacuaClient in Frameworks */,
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{tests_frameworks_id} /* Frameworks */ = {{
\t\t\tisa = PBXFrameworksBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t\t{tests_pkg_bf_id} /* VacuaClient in Frameworks */,
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{uitests_frameworks_id} /* Frameworks */ = {{
\t\t\tisa = PBXFrameworksBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
/* End PBXFrameworksBuildPhase section */

/* Begin PBXGroup section */
\t\t{main_group_id} = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
\t\t\t\t{vacua_group_id} /* Vacua */,
\t\t\t\t{vacua_tests_group_id} /* VacuaTests */,
\t\t\t\t{vacua_uitests_group_id} /* VacuaUITests */,
\t\t\t\t{products_group_id} /* Products */,
\t\t\t);
\t\t\tsourceTree = "<group>";
\t\t}};
\t\t{products_group_id} /* Products */ = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
\t\t\t\t{vacua_app_ref_id} /* Vacua.app */,
\t\t\t\t{vacua_tests_ref_id} /* VacuaTests.xctest */,
\t\t\t\t{vacua_uitests_ref_id} /* VacuaUITests.xctest */,
\t\t\t);
\t\t\tname = Products;
\t\t\tsourceTree = "<group>";
\t\t}};
\t\t{vacua_group_id} /* Vacua */ = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
\t\t\t\t{info_plist_id} /* Info.plist */,
\t\t\t\t{assets_ref_id} /* Assets.xcassets */,
\t\t\t\t{localizable_ref_id} /* Localizable.xcstrings */,
"""
    for f_id, _, _, name in app_file_entries:
        pbx += f"\t\t\t\t{f_id} /* {name} */,\n"
    pbx += f"""\t\t\t);
\t\t\tname = Vacua;
\t\t\tsourceTree = "<group>";
\t\t}};
\t\t{vacua_tests_group_id} /* VacuaTests */ = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
"""
    for f_id, _, _, name in test_file_entries:
        pbx += f"\t\t\t\t{f_id} /* {name} */,\n"
    pbx += f"""\t\t\t);
\t\t\tname = VacuaTests;
\t\t\tsourceTree = "<group>";
\t\t}};
\t\t{vacua_uitests_group_id} /* VacuaUITests */ = {{
\t\t\tisa = PBXGroup;
\t\t\tchildren = (
"""
    for f_id, _, _, name in uitest_file_entries:
        pbx += f"\t\t\t\t{f_id} /* {name} */,\n"
    pbx += f"""\t\t\t);
\t\t\tname = VacuaUITests;
\t\t\tsourceTree = "<group>";
\t\t}};
/* End PBXGroup section */

/* Begin PBXNativeTarget section */
\t\t{vacua_target_id} /* Vacua */ = {{
\t\t\tisa = PBXNativeTarget;
\t\t\tbuildConfigurationList = {vacua_cfg_list_id} /* Build configuration list for PBXNativeTarget "Vacua" */;
\t\t\tbuildPhases = (
\t\t\t\t{vacua_sources_id} /* Sources */,
\t\t\t\t{vacua_frameworks_id} /* Frameworks */,
\t\t\t\t{vacua_resources_id} /* Resources */,
\t\t\t);
\t\t\tbuildRules = (
\t\t\t);
\t\t\tdependencies = (
\t\t\t);
\t\t\tname = Vacua;
\t\t\tpackageProductDependencies = (
\t\t\t\t{package_product_vacua_id} /* VacuaClient */,
\t\t\t);
\t\t\tproductName = Vacua;
\t\t\tproductReference = {vacua_app_ref_id} /* Vacua.app */;
\t\t\tproductType = "com.apple.product-type.application";
\t\t}};
\t\t{vacua_tests_target_id} /* VacuaTests */ = {{
\t\t\tisa = PBXNativeTarget;
\t\t\tbuildConfigurationList = {tests_cfg_list_id} /* Build configuration list for PBXNativeTarget "VacuaTests" */;
\t\t\tbuildPhases = (
\t\t\t\t{tests_sources_id} /* Sources */,
\t\t\t\t{tests_frameworks_id} /* Frameworks */,
\t\t\t\t{tests_resources_id} /* Resources */,
\t\t\t);
\t\t\tbuildRules = (
\t\t\t);
\t\t\tdependencies = (
\t\t\t\t{target_dep_tests_id} /* PBXTargetDependency */,
\t\t\t);
\t\t\tname = VacuaTests;
\t\t\tpackageProductDependencies = (
\t\t\t\t{package_product_tests_id} /* VacuaClient */,
\t\t\t);
\t\t\tproductName = VacuaTests;
\t\t\tproductReference = {vacua_tests_ref_id} /* VacuaTests.xctest */;
\t\t\tproductType = "com.apple.product-type.bundle.unit-test";
\t\t}};
\t\t{vacua_uitests_target_id} /* VacuaUITests */ = {{
\t\t\tisa = PBXNativeTarget;
\t\t\tbuildConfigurationList = {uitests_cfg_list_id} /* Build configuration list for PBXNativeTarget "VacuaUITests" */;
\t\t\tbuildPhases = (
\t\t\t\t{uitests_sources_id} /* Sources */,
\t\t\t\t{uitests_frameworks_id} /* Frameworks */,
\t\t\t\t{uitests_resources_id} /* Resources */,
\t\t\t);
\t\t\tbuildRules = (
\t\t\t);
\t\t\tdependencies = (
\t\t\t\t{target_dep_uitests_id} /* PBXTargetDependency */,
\t\t\t);
\t\t\tname = VacuaUITests;
\t\t\tproductName = VacuaUITests;
\t\t\tproductReference = {vacua_uitests_ref_id} /* VacuaUITests.xctest */;
\t\t\tproductType = "com.apple.product-type.bundle.ui-testing";
\t\t}};
/* End PBXNativeTarget section */

/* Begin PBXProject section */
\t\t{project_id} /* Project object */ = {{
\t\t\tisa = PBXProject;
\t\t\tattributes = {{
\t\t\t\tBuildIndependentTargetsInParallel = 1;
\t\t\t\tLastUpgradeCheck = 1600;
\t\t\t\tTargetAttributes = {{
\t\t\t\t\t{vacua_target_id} = {{
\t\t\t\t\t\tCreatedOnToolsVersion = 16.0;
\t\t\t\t\t}};
\t\t\t\t\t{vacua_tests_target_id} = {{
\t\t\t\t\t\tCreatedOnToolsVersion = 16.0;
\t\t\t\t\t\tTestTargetID = {vacua_target_id};
\t\t\t\t\t}};
\t\t\t\t\t{vacua_uitests_target_id} = {{
\t\t\t\t\t\tCreatedOnToolsVersion = 16.0;
\t\t\t\t\t\tTestTargetID = {vacua_target_id};
\t\t\t\t\t}};
\t\t\t\t}};
\t\t\t}};
\t\t\tbuildConfigurationList = {proj_cfg_list_id} /* Build configuration list for PBXProject "Vacua" */;
\t\t\tcompatibilityVersion = "Xcode 14.0";
\t\t\tdevelopmentRegion = en;
\t\t\thasScannedForEncodings = 0;
\t\t\tknownRegions = (
\t\t\t\ten,
\t\t\t\tBase,
\t\t\t);
\t\t\tmainGroup = {main_group_id};
\t\t\tpackageReferences = (
\t\t\t\t{package_ref_id} /* XCLocalSwiftPackageReference "Packages/VacuaClient" */,
\t\t\t);
\t\t\tproductRefGroup = {products_group_id} /* Products */;
\t\t\tprojectDirPath = "";
\t\t\tprojectRoot = "";
\t\t\ttargets = (
\t\t\t\t{vacua_target_id} /* Vacua */,
\t\t\t\t{vacua_tests_target_id} /* VacuaTests */,
\t\t\t\t{vacua_uitests_target_id} /* VacuaUITests */,
\t\t\t);
\t\t}};
/* End PBXProject section */

/* Begin PBXResourcesBuildPhase section */
\t\t{vacua_resources_id} /* Resources */ = {{
\t\t\tisa = PBXResourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t\t{assets_bf_id} /* Assets.xcassets in Resources */,
\t\t\t\t{localizable_bf_id} /* Localizable.xcstrings in Resources */,
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{tests_resources_id} /* Resources */ = {{
\t\t\tisa = PBXResourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{uitests_resources_id} /* Resources */ = {{
\t\t\tisa = PBXResourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
/* End PBXResourcesBuildPhase section */

/* Begin PBXSourcesBuildPhase section */
\t\t{vacua_sources_id} /* Sources */ = {{
\t\t\tisa = PBXSourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
"""
    for _, bf_id, _, name in app_file_entries:
        pbx += f"\t\t\t\t{bf_id} /* {name} in Sources */,\n"
    pbx += f"""\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{tests_sources_id} /* Sources */ = {{
\t\t\tisa = PBXSourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
"""
    for _, bf_id, _, name in test_file_entries:
        pbx += f"\t\t\t\t{bf_id} /* {name} in Sources */,\n"
    pbx += f"""\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
\t\t{uitests_sources_id} /* Sources */ = {{
\t\t\tisa = PBXSourcesBuildPhase;
\t\t\tbuildActionMask = 2147483647;
\t\t\tfiles = (
"""
    for _, bf_id, _, name in uitest_file_entries:
        pbx += f"\t\t\t\t{bf_id} /* {name} in Sources */,\n"
    pbx += f"""\t\t\t);
\t\t\trunOnlyForDeploymentPostprocessing = 0;
\t\t}};
/* End PBXSourcesBuildPhase section */

/* Begin XCBuildConfiguration section */
\t\t{proj_cfg_debug_id} /* Debug */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tALWAYS_SEARCH_USER_PATHS = NO;
\t\t\t\tCLANG_ANALYZER_NONNULL = YES;
\t\t\t\tCLANG_ENABLE_MODULES = YES;
\t\t\t\tCLANG_ENABLE_OBJC_ARC = YES;
\t\t\t\tCODE_SIGNING_ALLOWED = NO;
\t\t\t\tCODE_SIGNING_REQUIRED = NO;
\t\t\t\tCOPY_PHASE_STRIP = NO;
\t\t\t\tDEBUG_INFORMATION_FORMAT = dwarf;
\t\t\t\tENABLE_TESTABILITY = YES;
\t\t\t\tGCC_DYNAMIC_NO_PIC = NO;
\t\t\t\tGCC_OPTIMIZATION_LEVEL = 0;
\t\t\t\tGCC_PREPROCESSOR_DEFINITIONS = (
\t\t\t\t\t"DEBUG=1",
\t\t\t\t\t"$(inherited)",
\t\t\t\t);
\t\t\t\tMACOSX_DEPLOYMENT_TARGET = 15.0;
\t\t\t\tMTL_ENABLE_DEBUG_INFO = INCLUDE_SOURCE;
\t\t\t\tONLY_ACTIVE_ARCH = YES;
\t\t\t\tSDKROOT = macosx;
\t\t\t\tSWIFT_ACTIVE_COMPILATION_CONDITIONS = DEBUG;
\t\t\t\tSWIFT_OPTIMIZATION_LEVEL = "-Onone";
\t\t\t\tSWIFT_VERSION = 6.0;
\t\t\t}};
\t\t\tname = Debug;
\t\t}};
\t\t{proj_cfg_release_id} /* Release */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tALWAYS_SEARCH_USER_PATHS = NO;
\t\t\t\tCLANG_ANALYZER_NONNULL = YES;
\t\t\t\tCLANG_ENABLE_MODULES = YES;
\t\t\t\tCLANG_ENABLE_OBJC_ARC = YES;
\t\t\t\tCODE_SIGNING_ALLOWED = NO;
\t\t\t\tCODE_SIGNING_REQUIRED = NO;
\t\t\t\tCOPY_PHASE_STRIP = NO;
\t\t\t\tDEBUG_INFORMATION_FORMAT = "dwarf-with-dsym";
\t\t\t\tENABLE_NS_ASSERTIONS = NO;
\t\t\t\tGCC_OPTIMIZATION_LEVEL = s;
\t\t\t\tMACOSX_DEPLOYMENT_TARGET = 15.0;
\t\t\t\tMTL_ENABLE_DEBUG_INFO = NO;
\t\t\t\tSDKROOT = macosx;
\t\t\t\tSWIFT_COMPILATION_MODE = "wholemodule";
\t\t\t\tSWIFT_OPTIMIZATION_LEVEL = "-O";
\t\t\t\tSWIFT_VERSION = 6.0;
\t\t\t}};
\t\t\tname = Release;
\t\t}};
\t\t{vacua_cfg_debug_id} /* Debug */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCOMBINE_HIDPI_IMAGES = YES;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tENABLE_HARDENED_RUNTIME = YES;
\t\t\t\tGENERATE_INFOPLIST_FILE = NO;
\t\t\t\tINFOPLIST_FILE = "Vacua/Resources/Info.plist";
\t\t\t\tLD_RUNPATH_SEARCH_PATHS = (
\t\t\t\t\t"$(inherited)",
\t\t\t\t\t"@executable_path/../Frameworks",
\t\t\t\t);
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = YES;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t}};
\t\t\tname = Debug;
\t\t}};
\t\t{vacua_cfg_release_id} /* Release */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tASSETCATALOG_COMPILER_APPICON_NAME = AppIcon;
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCOMBINE_HIDPI_IMAGES = YES;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tENABLE_HARDENED_RUNTIME = YES;
\t\t\t\tGENERATE_INFOPLIST_FILE = NO;
\t\t\t\tINFOPLIST_FILE = "Vacua/Resources/Info.plist";
\t\t\t\tLD_RUNPATH_SEARCH_PATHS = (
\t\t\t\t\t"$(inherited)",
\t\t\t\t\t"@executable_path/../Frameworks",
\t\t\t\t);
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = YES;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t}};
\t\t\tname = Release;
\t\t}};
\t\t{tests_cfg_debug_id} /* Debug */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tBUNDLE_LOADER = "$(TEST_HOST)";
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tGENERATE_INFOPLIST_FILE = YES;
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua.VacuaTests;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = NO;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t\tTEST_HOST = "$(BUILT_PRODUCTS_DIR)/Vacua.app/Contents/MacOS/Vacua";
\t\t\t}};
\t\t\tname = Debug;
\t\t}};
\t\t{tests_cfg_release_id} /* Release */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tBUNDLE_LOADER = "$(TEST_HOST)";
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tGENERATE_INFOPLIST_FILE = YES;
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua.VacuaTests;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = NO;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t\tTEST_HOST = "$(BUILT_PRODUCTS_DIR)/Vacua.app/Contents/MacOS/Vacua";
\t\t\t}};
\t\t\tname = Release;
\t\t}};
\t\t{uitests_cfg_debug_id} /* Debug */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tGENERATE_INFOPLIST_FILE = YES;
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua.VacuaUITests;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = NO;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t\tTEST_TARGET_NAME = Vacua;
\t\t\t}};
\t\t\tname = Debug;
\t\t}};
\t\t{uitests_cfg_release_id} /* Release */ = {{
\t\t\tisa = XCBuildConfiguration;
\t\t\tbuildSettings = {{
\t\t\t\tCODE_SIGN_STYLE = Manual;
\t\t\t\tCURRENT_PROJECT_VERSION = 1;
\t\t\t\tGENERATE_INFOPLIST_FILE = YES;
\t\t\t\tMARKETING_VERSION = 0.8.0;
\t\t\t\tPRODUCT_BUNDLE_IDENTIFIER = io.github.yuanweize.vacua.VacuaUITests;
\t\t\t\tPRODUCT_NAME = "$(TARGET_NAME)";
\t\t\t\tSWIFT_EMIT_LOC_STRINGS = NO;
\t\t\t\tSWIFT_STRICT_CONCURRENCY = complete;
\t\t\t\tTEST_TARGET_NAME = Vacua;
\t\t\t}};
\t\t\tname = Release;
\t\t}};
/* End XCBuildConfiguration section */

/* Begin XCConfigurationList section */
\t\t{proj_cfg_list_id} /* Build configuration list for PBXProject "Vacua" */ = {{
\t\t\tisa = XCConfigurationList;
\t\t\tbuildConfigurations = (
\t\t\t\t{proj_cfg_debug_id} /* Debug */,
\t\t\t\t{proj_cfg_release_id} /* Release */,
\t\t\t);
\t\t\tdefaultConfigurationIsVisible = 0;
\t\t\tdefaultConfigurationName = Release;
\t\t}};
\t\t{vacua_cfg_list_id} /* Build configuration list for PBXNativeTarget "Vacua" */ = {{
\t\t\tisa = XCConfigurationList;
\t\t\tbuildConfigurations = (
\t\t\t\t{vacua_cfg_debug_id} /* Debug */,
\t\t\t\t{vacua_cfg_release_id} /* Release */,
\t\t\t);
\t\t\tdefaultConfigurationIsVisible = 0;
\t\t\tdefaultConfigurationName = Release;
\t\t}};
\t\t{tests_cfg_list_id} /* Build configuration list for PBXNativeTarget "VacuaTests" */ = {{
\t\t\tisa = XCConfigurationList;
\t\t\tbuildConfigurations = (
\t\t\t\t{tests_cfg_debug_id} /* Debug */,
\t\t\t\t{tests_cfg_release_id} /* Release */,
\t\t\t);
\t\t\tdefaultConfigurationIsVisible = 0;
\t\t\tdefaultConfigurationName = Release;
\t\t}};
\t\t{uitests_cfg_list_id} /* Build configuration list for PBXNativeTarget "VacuaUITests" */ = {{
\t\t\tisa = XCConfigurationList;
\t\t\tbuildConfigurations = (
\t\t\t\t{uitests_cfg_debug_id} /* Debug */,
\t\t\t\t{uitests_cfg_release_id} /* Release */,
\t\t\t);
\t\t\tdefaultConfigurationIsVisible = 0;
\t\t\tdefaultConfigurationName = Release;
\t\t}};
/* End XCConfigurationList section */

/* Begin XCLocalSwiftPackageReference section */
\t\t{package_ref_id} /* XCLocalSwiftPackageReference "Packages/VacuaClient" */ = {{
\t\t\tisa = XCLocalSwiftPackageReference;
\t\t\trelativePath = Packages/VacuaClient;
\t\t}};
/* End XCLocalSwiftPackageReference section */

/* Begin XCSwiftPackageProductDependency section */
\t\t{package_product_vacua_id} /* VacuaClient */ = {{
\t\t\tisa = XCSwiftPackageProductDependency;
\t\t\tpackage = {package_ref_id} /* XCLocalSwiftPackageReference "Packages/VacuaClient" */;
\t\t\tproductName = VacuaClient;
\t\t}};
\t\t{package_product_tests_id} /* VacuaClient */ = {{
\t\t\tisa = XCSwiftPackageProductDependency;
\t\t\tpackage = {package_ref_id} /* XCLocalSwiftPackageReference "Packages/VacuaClient" */;
\t\t\tproductName = VacuaClient;
\t\t}};
/* End XCSwiftPackageProductDependency section */

\t}};
\trootObject = {project_id} /* Project object */;
}}
"""

    pbx_path = os.path.join(xcodeproj_dir, "project.pbxproj")
    with open(pbx_path, "w", encoding="utf-8") as f:
        f.write(pbx)
    print(f"Generated {pbx_path}")

    # Shared Scheme for Vacua
    scheme_xml = f"""<?xml version="1.0" encoding="UTF-8"?>
<Scheme
   LastUpgradeVersion = "1600"
   version = "1.7">
   <BuildAction
      parallelizeBuildables = "YES"
      buildImplicitDependencies = "YES">
      <BuildActionEntries>
         <BuildActionEntry
            buildForTesting = "YES"
            buildForRunning = "YES"
            buildForProfiling = "YES"
            buildForArchiving = "YES"
            buildForAnalyzing = "YES">
            <BuildableReference
               BuildableIdentifier = "primary"
               BlueprintIdentifier = "{vacua_target_id}"
               BuildableName = "Vacua.app"
               BlueprintName = "Vacua"
               ReferencedContainer = "container:Vacua.xcodeproj">
            </BuildableReference>
         </BuildActionEntry>
         <BuildActionEntry
            buildForTesting = "YES"
            buildForRunning = "NO"
            buildForProfiling = "NO"
            buildForArchiving = "NO"
            buildForAnalyzing = "NO">
            <BuildableReference
               BuildableIdentifier = "primary"
               BlueprintIdentifier = "{vacua_tests_target_id}"
               BuildableName = "VacuaTests.xctest"
               BlueprintName = "VacuaTests"
               ReferencedContainer = "container:Vacua.xcodeproj">
            </BuildableReference>
         </BuildActionEntry>
      </BuildActionEntries>
   </BuildAction>
   <TestAction
      buildConfiguration = "Debug"
      selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB"
      selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB"
      shouldUseLaunchSchemeArgsEnv = "YES">
      <Testables>
         <TestableReference
            skipped = "NO">
            <BuildableReference
               BuildableIdentifier = "primary"
               BlueprintIdentifier = "{vacua_tests_target_id}"
               BuildableName = "VacuaTests.xctest"
               BlueprintName = "VacuaTests"
               ReferencedContainer = "container:Vacua.xcodeproj">
            </BuildableReference>
         </TestableReference>
      </Testables>
   </TestAction>
   <LaunchAction
      buildConfiguration = "Debug"
      selectedDebuggerIdentifier = "Xcode.DebuggerFoundation.Debugger.LLDB"
      selectedLauncherIdentifier = "Xcode.DebuggerFoundation.Launcher.LLDB"
      launchStyle = "0"
      useCustomWorkingDirectory = "NO"
      ignoresPersistentStateOnLaunch = "NO"
      debugDocumentVersioning = "YES"
      debugServiceExtension = "internal"
      allowLocationSimulation = "YES">
      <BuildableProductRunnable
         runnableDebuggingMode = "0">
         <BuildableReference
            BuildableIdentifier = "primary"
            BlueprintIdentifier = "{vacua_target_id}"
            BuildableName = "Vacua.app"
            BlueprintName = "Vacua"
            ReferencedContainer = "container:Vacua.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </LaunchAction>
   <ProfileAction
      buildConfiguration = "Release"
      shouldUseLaunchSchemeArgsEnv = "YES"
      savedToolIdentifier = ""
      useCustomWorkingDirectory = "NO"
      debugDocumentVersioning = "YES">
      <BuildableProductRunnable
         runnableDebuggingMode = "0">
         <BuildableReference
            BuildableIdentifier = "primary"
            BlueprintIdentifier = "{vacua_target_id}"
            BuildableName = "Vacua.app"
            BlueprintName = "Vacua"
            ReferencedContainer = "container:Vacua.xcodeproj">
         </BuildableReference>
      </BuildableProductRunnable>
   </ProfileAction>
   <AnalyzeAction
      buildConfiguration = "Debug">
   </AnalyzeAction>
   <ArchiveAction
      buildConfiguration = "Release"
      revealArchiveInOrganizer = "YES">
   </ArchiveAction>
</Scheme>
"""
    scheme_path = os.path.join(shared_schemes_dir, "Vacua.xcscheme")
    with open(scheme_path, "w", encoding="utf-8") as f:
        f.write(scheme_xml)
    print(f"Generated {scheme_path}")

if __name__ == "__main__":
    main()
