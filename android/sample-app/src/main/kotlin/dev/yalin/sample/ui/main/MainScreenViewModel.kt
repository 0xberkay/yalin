package dev.yalin.sample.ui.main

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import dev.yalin.sample.data.DataRepository
import dev.yalin.sample.data.Dataset
import dev.yalin.sample.data.ParseResult
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch

data class MainScreenUiState(
  val selected: Dataset? = null,
  val isRunning: Boolean = false,
  val error: String? = null,
  val results: Map<Dataset, ParseResult> = emptyMap(),
)

class MainScreenViewModel(private val dataRepository: DataRepository) : ViewModel() {
  private val _uiState = MutableStateFlow(MainScreenUiState())
  val uiState = _uiState.asStateFlow()

  fun measure(dataset: Dataset) {
    if (_uiState.value.isRunning) return
    _uiState.update { it.copy(selected = dataset, isRunning = true, error = null) }
    viewModelScope.launch {
      try {
        val result = dataRepository.measure(dataset)
        _uiState.update { it.copy(isRunning = false, results = it.results + (dataset to result)) }
      } catch (e: CancellationException) {
        throw e
      } catch (e: Exception) {
        _uiState.update { it.copy(isRunning = false, error = e.localizedMessage ?: e.javaClass.simpleName) }
      }
    }
  }
}
